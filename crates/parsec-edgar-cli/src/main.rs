fn main() -> std::process::ExitCode {
    parsec_edgar_cli::run_from(std::env::args_os()).into()
}
