//! `zenvx-settings` — CLI stand-in for the settings panel; prints current
//! settings with the API key masked.

fn main() {
    let s = zenvx_shell_ui::Settings::load_or_default();
    println!("ZenvX settings");
    println!("  provider:  {}", s.provider());
    println!("  model:     {}", s.model().unwrap_or("(default)"));
    println!("  api key:   {}", s.masked_key());
}
