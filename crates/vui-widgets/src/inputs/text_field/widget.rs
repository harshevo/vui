//! The field as a widget: its state, measuring, painting, and routing events to editing and the
//! pointer.

use super::*;

impl<M: 'static> Widget<M> for TextFieldWidget<M> {
    type State = FieldState;

    fn create_state(&self) -> FieldState {
        let mut editor = Editor::new();
        editor.set_multiline(self.spec.multiline);
        editor.set_max_chars(self.spec.max_chars);
        editor.set_text(&self.value);
        FieldState {
            editor,
            scroll: Vec2::ZERO,
            focus: SpringValue::new(0.0, Spring::critical(600.0)),
            blink_epoch: 0.0,
            caret_on: true,
            drag: None,
            clear_hover: false,
            baseline: 0.0,
        }
    }

    fn update(&self, old: &TextFieldWidget<M>, state: &mut FieldState) -> Dirty {
        state.editor.set_multiline(self.spec.multiline);
        state.editor.set_max_chars(self.spec.max_chars);
        let mut dirty = Dirty::None;
        // The app changed the value, or a controlled field is being held to the value it passed.
        let outside = self.value != old.value || self.on_change.is_some();
        if outside && state.editor.text() != self.value {
            state.editor.set_text(&self.value);
            dirty = if self.spec.multiline && self.spec.max_rows.is_some() { Dirty::Layout } else { Dirty::Paint };
        }
        if self.spec != old.spec {
            dirty = Dirty::Layout;
        } else if self.placeholder != old.placeholder {
            dirty = dirty.max(Dirty::Paint);
        }
        dirty
    }

    fn measure(&self, state: &mut FieldState, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let theme = &env.theme;
        let style = self.style(theme);
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w.min(DEFAULT_WIDTH),
            Avail::MinContent => MIN_WIDTH,
            Avail::MaxContent => DEFAULT_WIDTH,
        });
        let height = if self.spec.multiline {
            let frame = self.frame(rect(0.0, 0.0, width, 1000.0), &style, env.scale, self.show_clear(&state.editor));
            let surface = Surface::new(&state.editor, env.fonts(), &style, Some(frame.inner.w), false);
            let lines = (surface.layout.size().h / style.line_height).round().max(1.0) as usize;
            let most = self.spec.max_rows.unwrap_or(self.spec.rows).max(self.spec.rows);
            known.y.unwrap_or(lines.clamp(self.spec.rows, most) as f32 * style.line_height + 2.0 * PAD_Y)
        } else {
            known.y.unwrap_or(if self.spec.large { theme.control_large } else { theme.control })
        };
        let probe = env.fonts().layout_cached("Ag", &style, None);
        let top = if self.spec.multiline { PAD_Y } else { (height - style.line_height) / 2.0 };
        state.baseline = top + probe.baseline();
        size(width, height)
    }

    fn baseline(&self, state: &FieldState) -> Option<f32> {
        Some(state.baseline)
    }

    fn paint(&self, state: &FieldState, cx: &mut PaintCx) {
        let palette = cx.palette().clone();
        let alpha = cx.alpha;
        let focus = state.focus.value().clamp(0.0, 1.0);

        // The border goes over to the accent as the field gains focus, and a faint halo grows around it.
        let edge = palette.line.lerp(palette.text_3, cx.hover * 0.7).lerp(palette.accent, focus);
        let (outline, radii, smooth) = (cx.rect, cx.radii, cx.smooth);
        if !self.spec.bare {
            cx.quad(Quad::new(outline).radii(radii).smooth(smooth).border(1.0, edge.mul_alpha(alpha)));
        }
        if focus > 0.01 && !self.spec.bare {
            let halo = Quad::new(outline.inflate(3.0, 3.0))
                .radii(Corners::all(radii.max() + 3.0))
                .smooth(smooth)
                .border(3.0, palette.accent.mul_alpha(0.16 * focus * alpha));
            cx.quad(halo);
        }

        let style = self.style(cx.theme());
        let scale = cx.env.scale;
        let frame = self.frame(cx.content, &style, scale, self.show_clear(&state.editor));
        // Wrap to the width the events and the measuring used, not the one moved onto the pixel grid.
        let wrap = self.spec.multiline.then(|| self.frame(cx.exact, &style, scale, self.show_clear(&state.editor)).inner.w);
        let surface = Surface::new(&state.editor, cx.env.fonts(), &style, wrap, self.spec.password);

        if let (Some(icon), Some(at)) = (self.spec.leading, frame.leading) {
            paint_icon(cx, icon, at, Tone::Text3, Weight::Regular);
        }
        if let Some(at) = frame.clear {
            paint_icon(cx, set::X_CIRCLE, at, if state.clear_hover { Tone::Text2 } else { Tone::Text3 }, Weight::Regular);
        }

        let inner = frame.inner;
        let scroll = self.clamp_scroll(state.scroll, &surface, inner);
        let origin = pt(inner.x - scroll.x, inner.y - scroll.y);
        let active = cx.focused;
        let key_window = cx.window_focused;
        let editor = &state.editor;
        cx.clipped(inner, |cx| {
            if active {
                let fill = if key_window { Fill::Tone(Tone::Accent, 0.28) } else { Fill::Tone(Tone::Text, 0.12) };
                for r in surface.selection_rects(editor) {
                    cx.fill_rounded(r.translate(vec2(origin.x, origin.y)), Corners::all(2.0), fill);
                }
            }
            if editor.is_empty() && editor.preedit().is_none() {
                if !self.placeholder.is_empty() {
                    let layout = cx.fonts().layout_cached(&self.placeholder, &style, wrap);
                    cx.text(layout, origin, Tone::Text3);
                }
            } else {
                cx.text(surface.layout.clone(), origin, Tone::Text);
            }
            // The text still being composed is underlined.
            for r in surface.preedit_rects(editor) {
                cx.fill_rounded(rect(origin.x + r.x, origin.y + r.bottom() - 3.0, r.w, 1.0), Corners::ZERO, Tone::Text);
            }
            if active && key_window && state.caret_on && !editor.has_selection() && !self.spec.read_only {
                let caret = surface.caret(editor);
                let height = (style.size * 1.3).round().min(caret.h);
                let x = ((origin.x + caret.x) * scale).round() / scale;
                let y = origin.y + caret.y + (caret.h - height) / 2.0;
                cx.fill_rounded(rect(x, y, CARET_W, height), Corners::all(CARET_W / 2.0), Tone::Accent);
            }
        });
    }

    fn event(&self, state: &mut FieldState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::FocusChanged { focused, by, .. } => {
                self.focus_changed(state, cx, *focused, *by);
                false
            }
            WidgetEvent::WindowFocus(on) => {
                if *on {
                    self.restart_blink(state, cx);
                }
                cx.request_paint();
                false
            }
            WidgetEvent::HoverChanged(false) => {
                if state.clear_hover {
                    state.clear_hover = false;
                    cx.request_paint();
                }
                false
            }
            WidgetEvent::Action(action) => self.action(state, cx, action),
            WidgetEvent::Input(input) => match input {
                Event::PointerDown { pos, button: PointerButton::Primary, mods, click_count } => {
                    self.pointer_down(state, cx, *pos, *mods, *click_count)
                }
                Event::PointerMoved { pos, .. } => self.pointer_moved(state, cx, *pos),
                Event::PointerUp { .. } => state.drag.take().is_some(),
                Event::Key(key) => self.key(state, cx, key),
                Event::Ime(ime) => self.ime(state, cx, ime),
                Event::Scroll { delta, .. } => self.scroll(state, cx, *delta),
                _ => false,
            },
            _ => false,
        }
    }

    fn tick(&self, state: &mut FieldState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = state.focus.step(dt);
        if cx.focused && cx.window_focused && !self.spec.read_only {
            let since = (cx.time - state.blink_epoch).max(0.0);
            let phase = (since / BLINK).floor();
            let on = phase as i64 % 2 == 0;
            if on != state.caret_on {
                state.caret_on = on;
                cx.request_paint();
            }
            cx.request_wake(((phase + 1.0) * BLINK - since).max(0.01));
        }
        moving
    }

    fn access(&self, state: &FieldState, info: &mut AccessInfo) {
        let editor = &state.editor;
        info.role = Some(if self.spec.search { Role::SearchInput } else { Role::TextInput });
        // The placeholder names the field when the app has not.
        if !self.placeholder.is_empty() {
            info.label = Some(self.placeholder.clone());
        }
        if self.spec.password {
            let count = |upto: usize| editor.text()[..upto].graphemes(true).count();
            info.value = Some(BULLET.to_string().repeat(count(editor.text().len())));
            info.selection = Some((count(editor.anchor()), count(editor.caret())));
        } else {
            info.value = Some(editor.text().to_string());
            info.selection = Some(editor.selection_chars());
        }
        if !self.spec.read_only {
            info.actions.push(AccessActionKind::SetValue);
        }
    }

    fn focusable(&self) -> Focus {
        Focus::Click
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }
}
