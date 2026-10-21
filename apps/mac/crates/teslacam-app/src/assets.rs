//! Embedded assets: the Lucide icons the UI uses, Barlow, and the top-down car render.
//!
//! Fonts and the car come from the same files as the web app (`packages/assets/fonts/`,
//! `apps/web/public/vehicle-top.png`), compiled in with `include_bytes!`.

use std::borrow::Cow;
use std::sync::Arc;

use gpui_kit::assets::{IconName, icon_assets};
use gpui_kit::{App, AssetSource, Image, ImageFormat, Result, SharedString};

icon_assets!(
    AppIcons,
    [
        Bookmark,
        Car,
        ChevronLeft,
        ChevronRight,
        Download,
        Eye,
        Film,
        Folder,
        LayoutGrid,
        MapPin,
        Route,
        Search,
        Shield,
        VideoOff,
        X,
    ]
);

/// The icons the UI draws (Lucide, the web app's icon set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Saved clips.
    Bookmark,
    /// `車両配置`
    Car,
    /// Previous.
    ChevronLeft,
    /// Next.
    ChevronRight,
    /// `書き出す`
    Download,
    /// Detection.
    Eye,
    /// `代表映像`
    Film,
    /// Folder chip and `フォルダを選ぶ`.
    Folder,
    /// `6分割`
    LayoutGrid,
    /// Place.
    MapPin,
    /// Recent drives.
    Route,
    /// Search.
    Search,
    /// Sentry.
    Shield,
    /// A camera without footage.
    VideoOff,
    /// Close.
    X,
}

impl Icon {
    /// The asset path.
    pub fn path(self) -> SharedString {
        let name = match self {
            Icon::Bookmark => IconName::Bookmark,
            Icon::Car => IconName::Car,
            Icon::ChevronLeft => IconName::ChevronLeft,
            Icon::ChevronRight => IconName::ChevronRight,
            Icon::Download => IconName::Download,
            Icon::Eye => IconName::Eye,
            Icon::Film => IconName::Film,
            Icon::Folder => IconName::Folder,
            Icon::LayoutGrid => IconName::LayoutGrid,
            Icon::MapPin => IconName::MapPin,
            Icon::Route => IconName::Route,
            Icon::Search => IconName::Search,
            Icon::Shield => IconName::Shield,
            Icon::VideoOff => IconName::VideoOff,
            Icon::X => IconName::X,
        };
        name.path()
    }
}

/// The app's icons first, then GPUI Kit's default component icons.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match AppIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = AppIcons.list(path)?;
        paths.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(paths)
    }
}

const BARLOW_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../packages/assets/fonts/Barlow-Medium.ttf"
));
const BARLOW_SEMIBOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../packages/assets/fonts/Barlow-SemiBold.ttf"
));
const VEHICLE_TOP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../web/public/vehicle-top.png"
));

/// The numeral font family registered by [`register_fonts`].
pub const NUMERAL_FAMILY: &str = "Barlow";

/// Registers Barlow 500 and 600.
pub fn register_fonts(cx: &mut App) {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(BARLOW_MEDIUM),
            Cow::Borrowed(BARLOW_SEMIBOLD),
        ])
        .expect("register the bundled Barlow fonts");
}

/// The top-down car render (360 × 720).
pub fn vehicle_image() -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Png, VEHICLE_TOP.to_vec()))
}
