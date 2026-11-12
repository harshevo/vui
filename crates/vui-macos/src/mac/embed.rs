//! Native views placed by layout: WKWebView for web pages, an AVPlayerLayer for video and PDFKit's
//! PDFView for documents, each in a holder view that cuts it to the part that shows.
//!
//! `Effect::Embeds` lists them after every frame that moved one. Views are kept by id, so a page
//! keeps its scroll position and a video its place while layout moves them; one whose kind changes
//! is made again. Set `VUI_DEBUG_EMBEDS=1` to have each placement printed.

use std::rc::Rc;

use objc2_av_foundation::{AVLayerVideoGravityResizeAspect, AVPlayer, AVPlayerItemDidPlayToEndTimeNotification, AVPlayerLayer};
use objc2_core_media::kCMTimeZero;
use objc2_foundation::{NSNotificationCenter, NSOperationQueue, NSURLRequest, NSURL};
use objc2_pdf_kit::{PDFDocument, PDFView};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};
use vui_core::{Embed, EmbedKind};

use super::*;

/// The platform view behind one embed.
enum Content {
    Web(Retained<WKWebView>),
    Video { view: Retained<NSView>, player: Retained<AVPlayer>, looping: Rc<Cell<bool>>, observer: Retained<ProtocolObject<dyn NSObjectProtocol>> },
    Pdf(Retained<PDFView>),
}

/// A native view on screen: the holder that clips it, the view, and what was last applied to it.
pub struct Native {
    holder: Retained<NSView>,
    content: Content,
    applied: EmbedKind,
}

impl Native {
    fn new(mtm: MainThreadMarker, kind: &EmbedKind) -> Native {
        let holder = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
        holder.setWantsLayer(true);
        if let Some(layer) = holder.layer() {
            layer.setMasksToBounds(true);
        }
        let content = match kind {
            EmbedKind::Web { url } => {
                // SAFETY: a fresh configuration and view, on the main thread.
                let web = unsafe {
                    let config = WKWebViewConfiguration::new(mtm);
                    WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), NSRect::ZERO, &config)
                };
                load(&web, url);
                holder.addSubview(&web);
                Content::Web(web)
            }
            EmbedKind::Video { url, playing, looping, muted } => {
                let view = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
                // SAFETY: plain AVFoundation calls on the main thread; the notification block only
                // touches the player, which it keeps alive, and the flag.
                let (player, observer, looping) = unsafe {
                    let player = AVPlayer::playerWithURL(&ns_url(url), mtm);
                    player.setMuted(*muted);
                    let layer = AVPlayerLayer::playerLayerWithPlayer(Some(&player));
                    if let Some(gravity) = AVLayerVideoGravityResizeAspect {
                        layer.setVideoGravity(gravity);
                    }
                    // A layer-hosting view: the layer follows the view's bounds.
                    view.setLayer(Some(&layer));
                    view.setWantsLayer(true);
                    let flag = Rc::new(Cell::new(*looping));
                    let (again, replay) = (flag.clone(), player.clone());
                    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                        if again.get() {
                            replay.seekToTime(kCMTimeZero);
                            replay.play();
                        }
                    });
                    let observer = NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                        Some(AVPlayerItemDidPlayToEndTimeNotification),
                        player.currentItem().as_deref().map(|i| i.as_ref()),
                        Some(&NSOperationQueue::mainQueue()),
                        &block,
                    );
                    if *playing {
                        player.play();
                    }
                    (player, observer, flag)
                };
                holder.addSubview(&view);
                Content::Video { view, player, looping, observer }
            }
            EmbedKind::Pdf { url, page } => {
                // SAFETY: PDFKit calls on the main thread.
                let pdf = unsafe {
                    let pdf = PDFView::initWithFrame(PDFView::alloc(mtm), NSRect::ZERO);
                    pdf.setAutoScales(true);
                    pdf
                };
                open_pdf(&pdf, url, *page);
                holder.addSubview(&pdf);
                Content::Pdf(pdf)
            }
        };
        Native { holder, content, applied: kind.clone() }
    }

    fn same_kind(&self, kind: &EmbedKind) -> bool {
        std::mem::discriminant(&self.applied) == std::mem::discriminant(kind)
            && !matches!((&self.applied, kind), (EmbedKind::Video { url: a, .. }, EmbedKind::Video { url: b, .. }) if a != b)
    }

    /// Brings the view up to date with `kind`: a new page, play or pause, another PDF page.
    fn update(&mut self, kind: &EmbedKind) {
        match (&self.content, &self.applied, kind) {
            (Content::Web(web), EmbedKind::Web { url: was }, EmbedKind::Web { url }) if was != url => load(web, url),
            (Content::Video { player, looping: flag, .. }, EmbedKind::Video { playing: was, muted: was_muted, .. }, EmbedKind::Video { playing, looping, muted, .. }) => {
                flag.set(*looping);
                // SAFETY: AVPlayer calls on the main thread.
                unsafe {
                    if muted != was_muted {
                        player.setMuted(*muted);
                    }
                    match (was, playing) {
                        (false, true) => player.play(),
                        (true, false) => player.pause(),
                        _ => {}
                    }
                }
            }
            (Content::Pdf(pdf), EmbedKind::Pdf { url: was, page: was_page }, EmbedKind::Pdf { url, page }) if was != url || was_page != page => {
                if was != url {
                    open_pdf(pdf, url, *page);
                } else {
                    go_to(pdf, *page);
                }
            }
            _ => {}
        }
        self.applied = kind.clone();
    }

    /// Puts it where `e` says, in the coordinates of the flipped vui view, cut to what shows.
    fn place(&self, e: &Embed) {
        let Some(shown) = e.visible() else {
            self.holder.setHidden(true);
            return;
        };
        self.holder.setHidden(false);
        self.holder.setFrame(ns_rect(shown));
        self.holder.setAlphaValue(e.opacity as f64);
        if let Some(layer) = self.holder.layer() {
            layer.setCornerRadius(if shown == e.rect { e.radius as f64 } else { 0.0 });
        }
        // The holder is not flipped: y runs up from its bottom edge.
        let (x, top) = (e.rect.x - shown.x, e.rect.y - shown.y);
        let inner = NSRect::new(NSPoint::new(x as f64, (shown.h - top - e.rect.h) as f64), NSSize::new(e.rect.w as f64, e.rect.h as f64));
        let view: &NSView = match &self.content {
            Content::Web(web) => web,
            Content::Video { view, .. } => view,
            Content::Pdf(pdf) => pdf,
        };
        view.setFrame(inner);
    }

    fn remove(self) {
        if let Content::Video { player, observer, .. } = &self.content {
            // SAFETY: main-thread AVFoundation and Foundation calls; the observer came from this centre.
            unsafe {
                player.pause();
                NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref());
            }
        }
        self.holder.removeFromSuperview();
    }
}

impl VuiView {
    /// Shows the native views a frame asks for: new ones are made, the others moved, the rest removed.
    pub(super) fn set_embeds(&self, list: &[Embed]) {
        let mtm = self.mtm();
        let debug = std::env::var_os("VUI_DEBUG_EMBEDS").is_some();
        let mut old = self.ivars().embeds.take();
        let mut next = Vec::with_capacity(list.len());
        for e in list {
            let kept = old.iter().position(|(id, n)| *id == e.id && n.same_kind(&e.kind)).map(|i| old.remove(i).1);
            let mut native = kept.unwrap_or_else(|| {
                let native = Native::new(mtm, &e.kind);
                self.addSubview(&native.holder);
                native
            });
            native.update(&e.kind);
            native.place(e);
            if debug {
                let window = self.window().map_or(0, |w| w.windowNumber());
                eprintln!("vui: window {window} embed {} {:?} at {:?} showing {:?}", e.id, e.kind, e.rect, e.visible());
            }
            next.push((e.id, native));
        }
        for (_, native) in old {
            native.remove();
        }
        self.ivars().embeds.replace(next);
    }
}

/// A URL from a URL string, or from a file path.
fn ns_url(s: &str) -> Retained<NSURL> {
    let text = NSString::from_str(s);
    if s.contains("://")
        && let Some(url) = NSURL::URLWithString(&text)
    {
        return url;
    }
    NSURL::fileURLWithPath(&text)
}

fn load(web: &WKWebView, url: &str) {
    let ns = ns_url(url);
    // SAFETY: WebKit calls on the main thread.
    unsafe {
        if ns.isFileURL() {
            let folder = ns.URLByDeletingLastPathComponent().unwrap_or_else(|| ns.clone());
            web.loadFileURL_allowingReadAccessToURL(&ns, &folder);
        } else {
            web.loadRequest(&NSURLRequest::requestWithURL(&ns));
        }
    }
}

fn open_pdf(pdf: &PDFView, url: &str, page: usize) {
    // SAFETY: PDFKit calls on the main thread.
    unsafe {
        let doc = PDFDocument::initWithURL(PDFDocument::alloc(), &ns_url(url));
        pdf.setDocument(doc.as_deref());
    }
    go_to(pdf, page);
}

fn go_to(pdf: &PDFView, page: usize) {
    // SAFETY: PDFKit calls on the main thread; the page belongs to the view's document.
    unsafe {
        if let Some(doc) = pdf.document()
            && let Some(p) = doc.pageAtIndex(page.min(doc.pageCount().saturating_sub(1)))
        {
            pdf.goToPage(&p);
        }
    }
}

fn ns_rect(r: Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x as f64, r.y as f64), NSSize::new(r.w as f64, r.h as f64))
}
