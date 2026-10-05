use dalan_acp::SUPPORTED_PROTOCOL_VERSION;
use dalan_drivers::{PLANNED_DRIVERS, validate_driver_catalog};

fn main() {
    if let Err(error) = validate_driver_catalog(&PLANNED_DRIVERS) {
        eprintln!("Invalid driver catalog: {error}");
        std::process::exit(1);
    }
    println!(
        "Dalan: experimental MySQL/MariaDB sources, multi-table tabs and read-only query consoles."
    );
    println!("Desktop target: macOS first; Linux next; Windows later.");
    println!(
        "AI boundary: ACP v{SUPPORTED_PROTOCOL_VERSION} only; no app BYOK. Transport not implemented."
    );
    for driver in PLANNED_DRIVERS {
        println!(
            "{}: {:?} ({})",
            driver.engine.display_name(),
            driver.status,
            driver.proposed_backend
        );
    }
    println!("Read docs/product-plan.md and docs/development.md before implementing a feature.");
}
