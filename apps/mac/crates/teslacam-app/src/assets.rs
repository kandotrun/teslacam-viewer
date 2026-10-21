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
        CircleAlert,
        CircleCheck,
        Download,
        Eye,
        Film,
        Folder,
        FolderOpen,
        LayoutGrid,
        LoaderCircle,
        MapPin,
        Route,
        Search,
        Shield,
        VideoOff,
        X,
    ]
);

/// Icons drawn differently from the Lucide originals, like the web does: play and pause filled
/// (`fill="currentColor" strokeWidth={0}`), the skip arrows with a 1.6 stroke.
const CUSTOM_ICONS: [(&str, &str); 4] = [
    (
        "app/play.svg",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z" fill="#000"/></svg>"##,
    ),
    (
        "app/pause.svg",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#000"><rect x="14" y="3" width="5" height="18" rx="1"/><rect x="5" y="3" width="5" height="18" rx="1"/></svg>"##,
    ),
    (
        "app/rotate-ccw.svg",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>"##,
    ),
    (
        "app/rotate-cw.svg",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/></svg>"##,
    ),
];

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
    /// An export error (`もう一度`).
    CircleAlert,
    /// `書き出し完了`.
    CircleCheck,
    /// `Finderで表示`.
    FolderOpen,
    /// The loading spinner.
    LoaderCircle,
    /// Play (filled).
    Play,
    /// Pause (filled).
    Pause,
    /// `10秒戻る`.
    SkipBack,
    /// `10秒進む`.
    SkipForward,
}

impl Icon {
    /// The asset path.
    pub fn path(self) -> SharedString {
        let name = match self {
            Icon::Play => return CUSTOM_ICONS[0].0.into(),
            Icon::Pause => return CUSTOM_ICONS[1].0.into(),
            Icon::SkipBack => return CUSTOM_ICONS[2].0.into(),
            Icon::SkipForward => return CUSTOM_ICONS[3].0.into(),
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
            Icon::CircleAlert => IconName::CircleAlert,
            Icon::CircleCheck => IconName::CircleCheck,
            Icon::FolderOpen => IconName::FolderOpen,
            Icon::LoaderCircle => IconName::LoaderCircle,
        };
        name.path()
    }
}

/// The app's icons first, then GPUI Kit's default component icons.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, svg)) = CUSTOM_ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(svg.as_bytes())));
        }
        match AppIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths: Vec<SharedString> = CUSTOM_ICONS
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| name.starts_with(path))
            .map(SharedString::from)
            .collect();
        paths.extend(AppIcons.list(path)?);
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
