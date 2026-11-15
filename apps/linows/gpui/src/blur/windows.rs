//! Frost on Windows: one sprite per card on a composition layer under gpui's
//! own, painted with the host backdrop brush and clipped to the card's rounded
//! shape. The DWM window backdrop cannot do this: it blurs the whole window.

use std::cell::{OnceCell, RefCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Bounds, Pixels, Window};
use windows::System::DispatcherQueueController;
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{
    CompositionBackdropBrush, CompositionRoundedRectangleGeometry, Compositor, ContainerVisual,
    SpriteVisual,
};
use windows::UI::ViewManagement::UISettings;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWA_USE_HOSTBACKDROPBRUSH, DwmSetWindowAttribute};
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, USER_DEFAULT_SCREEN_DPI, WS_EX_NOREDIRECTIONBITMAP,
};
use windows::core::{BOOL, Interface, Result};
use windows_numerics::{Vector2, Vector3};

/// One card in logical pixels, as gpui laid it out.
#[derive(Clone, Copy, PartialEq)]
pub struct Patch {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
}

pub fn mark(frame: &mut Vec<Patch>, card: &Bounds<Pixels>, radius: f32) {
    frame.push(Patch {
        x: card.origin.x.into(),
        y: card.origin.y.into(),
        width: card.size.width.into(),
        height: card.size.height.into(),
        radius,
    });
}

static SUPPORTED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// A compositor needs a dispatcher queue on its thread and gpui makes
    /// none. A thread gets one for life, so it is never dropped.
    static QUEUE: OnceCell<Option<DispatcherQueueController>> = const { OnceCell::new() };
    static LAYER: RefCell<Option<Layer>> = const { RefCell::new(None) };
}

struct Card {
    visual: SpriteVisual,
    shape: CompositionRoundedRectangleGeometry,
}

impl Card {
    /// Edges land on device pixels, where gpui paints the card's own.
    fn place(&self, patch: &Patch, scale: f32) -> Result<()> {
        let edge = |v: f32| (v * scale).round();
        let (left, top) = (edge(patch.x), edge(patch.y));
        let size = Vector2 {
            X: edge(patch.x + patch.width) - left,
            Y: edge(patch.y + patch.height) - top,
        };
        let radius = (patch.radius * scale).min(size.X / 2.0).min(size.Y / 2.0);
        self.visual.SetOffset(Vector3 {
            X: left,
            Y: top,
            Z: 0.0,
        })?;
        self.visual.SetSize(size)?;
        self.shape.SetSize(size)?;
        self.shape.SetCornerRadius(Vector2 {
            X: radius,
            Y: radius,
        })
    }
}

struct Layer {
    hwnd: HWND,
    compositor: Compositor,
    /// Holds the layer on the window.
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    brush: CompositionBackdropBrush,
    cards: Vec<Card>,
    /// What the cards show now, so a prepaint that moved nothing costs nothing.
    patches: Vec<Patch>,
    scale: f32,
}

/// The "Transparency effects" switch in Windows settings. Off, the system
/// blurs nothing and Look stays clear glass.
fn transparency_on() -> bool {
    UISettings::new()
        .and_then(|settings| settings.AdvancedEffectsEnabled())
        .unwrap_or(false)
}

impl Layer {
    /// `None` when Windows has transparency off, or when gpui draws through
    /// a redirection bitmap (`GPUI_DISABLE_DIRECT_COMPOSITION`), which would
    /// cover the layer.
    fn new(hwnd: HWND) -> Result<Option<Self>> {
        if !transparency_on() {
            return Ok(None);
        }
        let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        if ex_style & WS_EX_NOREDIRECTIONBITMAP.0 == 0 {
            return Ok(None);
        }
        // Windows 11 only: on 10 this fails and Look stays clear glass.
        let on = BOOL::from(true);
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_HOSTBACKDROPBRUSH,
                &on as *const BOOL as *const c_void,
                size_of::<BOOL>() as u32,
            )?;
        }
        QUEUE.with(|queue| {
            queue.get_or_init(|| {
                let options = DispatcherQueueOptions {
                    dwSize: size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_NONE,
                };
                unsafe { CreateDispatcherQueueController(options) }.ok()
            });
        });
        let compositor = Compositor::new()?;
        // Not topmost: under gpui's swap chain, which holds the topmost slot.
        let target = unsafe {
            compositor
                .cast::<ICompositorDesktopInterop>()?
                .CreateDesktopWindowTarget(hwnd, false)?
        };
        let root = compositor.CreateContainerVisual()?;
        target.SetRoot(&root)?;
        let brush = compositor.CreateHostBackdropBrush()?;
        Ok(Some(Self {
            hwnd,
            compositor,
            _target: target,
            root,
            brush,
            cards: Vec::new(),
            patches: Vec::new(),
            scale: 0.0,
        }))
    }

    fn card(&self) -> Result<Card> {
        let visual = self.compositor.CreateSpriteVisual()?;
        let shape = self.compositor.CreateRoundedRectangleGeometry()?;
        visual.SetBrush(&self.brush)?;
        visual.SetClip(&self.compositor.CreateGeometricClipWithGeometry(&shape)?)?;
        Ok(Card { visual, shape })
    }

    fn set(&mut self, patches: Vec<Patch>) -> Result<()> {
        let scale = unsafe { GetDpiForWindow(self.hwnd) } as f32 / USER_DEFAULT_SCREEN_DPI as f32;
        if scale == self.scale && patches == self.patches {
            return Ok(());
        }
        let children = self.root.Children()?;
        while self.cards.len() > patches.len() {
            if let Some(card) = self.cards.pop() {
                children.Remove(&card.visual)?;
            }
        }
        while self.cards.len() < patches.len() {
            let card = self.card()?;
            children.InsertAtTop(&card.visual)?;
            self.cards.push(card);
        }
        for (card, patch) in self.cards.iter().zip(&patches) {
            card.place(patch, scale)?;
        }
        self.patches = patches;
        self.scale = scale;
        Ok(())
    }
}

/// Call on every open, from the main thread. The window is kept across
/// hides, so this builds the layer once.
pub fn attach_window(window: &Window) {
    let Some(hwnd) = crate::host::hwnd(window) else {
        return;
    };
    LAYER.with_borrow_mut(|layer| {
        if layer.as_ref().is_some_and(|layer| layer.hwnd == hwnd) {
            return;
        }
        // The old target goes first: a window carries one per slot.
        *layer = None;
        *layer = Layer::new(hwnd).unwrap_or_else(|err| {
            eprintln!("frost: {err}");
            None
        });
        SUPPORTED.store(layer.is_some(), Ordering::Relaxed);
    });
}

pub fn detach() {
    LAYER.take();
}

pub fn is_supported() -> bool {
    SUPPORTED.load(Ordering::Relaxed)
}

pub fn set_region(patches: Vec<Patch>) {
    LAYER.with_borrow_mut(|layer| {
        let Some(layer) = layer else {
            return;
        };
        if let Err(err) = layer.set(patches) {
            eprintln!("frost: {err}");
        }
    });
}
