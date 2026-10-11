#![forbid(unsafe_code)]
use clap::Parser;
use std::path::PathBuf;
#[derive(Debug, Parser)]
#[command(
    name = "cookie-heap-lab",
    version,
    about = "64-bit heap and memory-corruption educational game"
)]
struct Cli {
    /// Enable the integrated memory debugger
    #[arg(long)]
    debug: bool,
    /// Load game state from FILE instead of starting a fresh game
    #[arg(long, value_name = "FILE")]
    load: Option<PathBuf>,
    /// Seed a fresh deterministic game instance
    #[arg(long, value_name = "SEED", conflicts_with = "load")]
    seed: Option<u64>,
}
impl Cli {
    fn machine(&self) -> Result<cookie_game::Machine, String> {
        if let Some(path) = &self.load {
            save_state::load(path).map_err(|e| format!("cannot load {}: {e}", path.display()))
        } else {
            let seed = self.seed.unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64
            });
            cookie_game::Machine::new(seed).map_err(|e| e.to_string())
        }
    }
}
fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let machine = match cli.machine() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 850.0])
            .with_min_inner_size([360.0, 480.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let window_title = cookie_ui::window_title();
    match eframe::run_native(
        &window_title,
        options,
        Box::new(move |cc| Ok(Box::new(cookie_ui::CookieApp::new(cc, machine, cli.debug)))),
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    #[test]
    fn help_and_gating() {
        let help = Cli::command().render_long_help().to_string();
        for flag in ["--debug", "--load", "--seed", "--version"] {
            assert!(help.contains(flag));
        }
        assert!(!Cli::try_parse_from(["app"]).unwrap().debug);
        assert!(Cli::try_parse_from(["app", "--debug"]).unwrap().debug);
    }
    #[test]
    fn seed_conflicts_with_load() {
        assert!(Cli::try_parse_from(["app", "--seed", "1234", "--load", "test.chls"]).is_err());
    }
    #[test]
    fn missing_save_error() {
        let c = Cli::try_parse_from(["app", "--load", "/does/not/exist.chls"]).unwrap();
        assert!(
            c.machine()
                .unwrap_err()
                .contains("cannot load /does/not/exist.chls")
        );
    }
    #[test]
    fn explicit_seed_is_deterministic() {
        let c = Cli::try_parse_from(["app", "--seed", "1234"]).unwrap();
        assert_eq!(c.machine().unwrap(), c.machine().unwrap());
    }
}
