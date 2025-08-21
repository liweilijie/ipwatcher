use anyhow::{Context, Result};
use ipwatcher::{load_from, query_external_ip, get_last_ip, init_db, save_ip, Config, SmtpConfig};
use lettre::{
    message::{header, Mailbox, Message},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use reqwest::Client;
use std::net::IpAddr;
use time::OffsetDateTime;
use tokio::{signal, time::{sleep, Duration}};
use tracing::{info, error, trace};
use tracing_subscriber::{EnvFilter, fmt};


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber (prints to stdout by default)
    // Priority: RUST_LOG env -> fallback to global "trace"
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_target(false)       // show target/module
        .with_thread_ids(true)   // optional: show thread id
        .with_thread_names(true) // optional: show thread name
        .compact()               // tighter output
        .init();

    info!("ip-watcher starting...");
    trace!("tracing initialized at TRACE level");

    // ---- STARTUP JITTER: random delay 3–6 minutes to avoid boot-time network issues ----
    let jitter_secs: u64 = rand::random_range(3*60..=6*60);
    info!("Startup jitter: sleeping {} seconds before first network work.", jitter_secs);
    sleep(Duration::from_secs(jitter_secs)).await;


    // 1) Load config
    let cfg = load_from("config.toml").context("Failed to load config.toml")?;

    // 2) Init DB
    let conn = init_db(&cfg.db_path)?;

    // 3) Prepare mailer & HTTP client
    let mailer = build_mailer(&cfg.smtp)?;
    let http = Client::builder().user_agent("ip-watcher/0.2").build()?;

    // 4) Initial IP check
    let current_ip = query_external_ip(&http, cfg.ip_sources.clone()).await?;
    let last_ip = get_last_ip(&conn)?;

    if last_ip.is_none() {
        save_ip(&conn, current_ip)?;
        send_ip_email(&mailer, &cfg.smtp, current_ip, true).await?;
        info!("First detected external IP: {}, email sent.", current_ip);
    } else if Some(current_ip) != last_ip {
        save_ip(&conn, current_ip)?;
        send_ip_email(&mailer, &cfg.smtp, current_ip, false).await?;
        info!(
            "External IP changed: {:?} -> {}, email sent.",
            last_ip.unwrap(),
            current_ip
        );
    } else {
        info!("External IP unchanged: {}.", current_ip);
    }

    // 5) Periodic loop (graceful Ctrl+C)
    let interval = Duration::from_secs(cfg.check_interval_secs.max(30));
    info!("Entering polling loop (every {}s).", interval.as_secs());

    tokio::select! {
        _ = poll_loop(&http, &conn, &mailer, &cfg, interval) => {},
        _ = signal::ctrl_c() => {
            info!("\nCtrl+C received, shutting down.");
        }
    }

    Ok(())
}

async fn poll_loop(
    http: &Client,
    conn: &rusqlite::Connection,
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    cfg: &Config,
    interval: Duration,
) {
    loop {
        sleep(interval).await;

        match query_external_ip(http, cfg.ip_sources.clone()).await {
            Ok(ip) => match get_last_ip(conn) {
                Ok(last) => {
                    if Some(ip) != last {
                        if let Err(e) = save_ip(conn, ip) {
                            error!("Failed to save IP: {e:#}");
                            continue;
                        }
                        if let Err(e) = send_ip_email(mailer, &cfg.smtp, ip, false).await {
                            error!("Failed to send email: {e:#}");
                        } else {
                            info!("IP changed, email sent: {}", ip);
                        }
                    } else {
                        info!("IP unchanged: {}", ip);
                    }
                }
                Err(e) => error!("DB read error: {e:#}"),
            },
            Err(e) => error!("External IP query failed: {e:#}"),
        }
    }
}

fn build_mailer(cfg: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    // Be tolerant of spaces pasted into the app password
    let creds = Credentials::new(cfg.username.clone(), cfg.app_password.replace(' ', ""));
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.server)?
        .port(cfg.port)
        .credentials(creds)
        .build();
    Ok(mailer)
}

async fn send_ip_email(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    cfg: &SmtpConfig,
    ip: IpAddr,
    first_time: bool,
) -> Result<()> {
    let subject = if first_time {
        format!("[IP Watcher] First external IP detected: {}", ip)
    } else {
        format!("[IP Watcher] External IP changed: {}", ip)
    };
    let html = format!(
        r#"<p>Time: {time}</p>
<p>Current external IP: <b>{ip}</b></p>
<p>This email was sent automatically by ip-watcher.</p>"#,
        time = now_iso(),
        ip = ip
    );

    let email = Message::builder()
        .from(Mailbox::new(None, cfg.from.parse()?))
        .to(Mailbox::new(None, cfg.to.parse()?))
        .subject(subject)
        .header(header::ContentType::TEXT_HTML)
        .body(html)?;

    mailer.send(email).await.context("SMTP send failed")?;
    Ok(())
}

fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".into())
}
