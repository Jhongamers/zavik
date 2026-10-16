//! VENDOR tool — generates the signing key pair and mints
//! licenses. It is an `example`: it is NOT part of the shipped `devforge` binary.
//!
//! Usage:
//!   DF_SIGNING_KEY=<seed> cargo run --example license_tool -- \
//!       mint --plan team --holder "Acme" --expires 2027-01-01
//!
//!   cargo run --example license_tool -- genkey     # generates a new pair
//!
//! The private key (seed) lives in `DF_SIGNING_KEY` and NEVER goes to the repo.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use std::env;

fn b64u(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        // Generates a new pair. The (private) seed comes out here ONCE and goes to
        // the vendor's vault; the public one is embedded in the binary.
        Some("genkey") => {
            let mut rng = rand::rngs::OsRng;
            let signing = SigningKey::generate(&mut rng);
            println!("DF_SIGNING_KEY={}", b64u(&signing.to_bytes()));
            println!("PUBLIC_KEY_B64={}", b64u(signing.verifying_key().as_bytes()));
        }

        // Mints a license from the seed in DF_SIGNING_KEY.
        Some("mint") => {
            let plan = flag(&args, "--plan").unwrap_or("team");
            let holder = flag(&args, "--holder").unwrap_or("unknown");
            let expires = flag(&args, "--expires").unwrap_or("2027-01-01");

            let seed_b64 = env::var("DF_SIGNING_KEY")
                .expect("exporte DF_SIGNING_KEY=<seed b64url> antes de cunhar");
            let seed: [u8; 32] = URL_SAFE_NO_PAD
                .decode(seed_b64.trim())
                .expect("seed em base64url inválido")
                .try_into()
                .expect("seed deve ter 32 bytes");
            let signing = SigningKey::from_bytes(&seed);

            // Payload SEMANTICALLY equal to what the binary expects.
            let payload = serde_json::json!({
                "plan": plan,
                "holder": holder,
                "issued": chrono_today(),
                "expires": expires,
            });
            let payload_bytes = serde_json::to_vec(&payload).expect("payload serializa");
            let signature = signing.sign(&payload_bytes);

            println!(
                "DF1.{}.{}",
                b64u(&payload_bytes),
                b64u(&signature.to_bytes())
            );
        }

        _ => {
            eprintln!("uso: license_tool genkey | mint --plan <p> --holder <h> --expires <YYYY-MM-DD>");
            std::process::exit(2);
        }
    }
}

/// Today's date in ISO — no date dep (same algorithm as the CLI).
fn chrono_today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
