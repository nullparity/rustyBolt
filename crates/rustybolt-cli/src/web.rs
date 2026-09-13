/// The dashboard page. `<!--STYLES-->`, `<!--LOGO_SVG-->`, `<!--VERSION-->`,
/// `<!--LANG-->` and `/*INITIAL_STATE*/` are filled in when it is served.
pub const HTML_PAGE: &str = include_str!("../ui/index.html");

/// The stylesheet that build.rs compiled from ui/app.css.
pub const STYLESHEET: &str = include_str!(concat!(env!("OUT_DIR"), "/app.css"));
