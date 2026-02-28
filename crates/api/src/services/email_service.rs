use html_escape::encode_text;
use serde_json::json;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("API error (status {status}): {body}")]
    ApiError { status: u16, body: String },
}

#[derive(Clone)]
pub struct EmailConfig {
    pub scw_secret_key: String,
    pub scw_project_id: String,
    pub scw_region: String,
    pub sender_email: String,
    pub sender_name: String,
    pub frontend_base_url: String,
}

impl EmailConfig {
    pub fn from_env() -> Option<Self> {
        let scw_secret_key = std::env::var("SCW_SECRET_KEY").ok()?;
        let scw_project_id = std::env::var("SCW_DEFAULT_PROJECT_ID").ok()?;
        let sender_email = std::env::var("SCW_SENDER_EMAIL").ok()?;

        Some(Self {
            scw_secret_key,
            scw_project_id,
            scw_region: std::env::var("SCW_REGION").unwrap_or_else(|_| "fr-par".to_string()),
            sender_email,
            sender_name: std::env::var("SCW_SENDER_NAME")
                .unwrap_or_else(|_| "PocketPair".to_string()),
            frontend_base_url: std::env::var("FRONTEND_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
        })
    }
}

#[derive(Clone)]
pub struct EmailService {
    config: EmailConfig,
    client: reqwest::Client,
}

// ── Locale ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default)]
pub enum Locale {
    #[default]
    En,
    Fr,
    Nl,
}

impl Locale {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.get(..2).unwrap_or("en") {
            "fr" => Self::Fr,
            "nl" => Self::Nl,
            _ => Self::En,
        }
    }
}

// ── i18n strings ────────────────────────────────────────────────────

struct I18n {
    hi: &'static str,
    good_luck: &'static str,
    footer_tagline: &'static str,

    // Password reset
    pw_subject: &'static str,
    pw_heading: &'static str,
    pw_body: &'static str,
    pw_cta: &'static str,
    pw_disclaimer: &'static str,

    // Registration confirmed
    reg_subject_prefix: &'static str,
    reg_heading: &'static str,
    reg_body_tpl: &'static str,

    // Waitlist promoted
    wl_subject_prefix: &'static str,
    wl_heading: &'static str,
    wl_body_tpl: &'static str,

    // Tournament starting soon
    soon_subject_suffix: &'static str,
    soon_heading: &'static str,
    soon_body_tpl: &'static str,
    soon_ready: &'static str,
}

fn i18n(locale: Locale) -> &'static I18n {
    match locale {
        Locale::En => &I18N_EN,
        Locale::Fr => &I18N_FR,
        Locale::Nl => &I18N_NL,
    }
}

static I18N_EN: I18n = I18n {
    hi: "Hi",
    good_luck: "Good luck at the tables.",
    footer_tagline: "Poker Tournament Management",

    pw_subject: "Reset Your Password",
    pw_heading: "Password Reset",
    pw_body: "We received a request to reset your password. Use the button below to choose a new one:",
    pw_cta: "Reset Password",
    pw_disclaimer: "This link expires in 1 hour. If you didn&rsquo;t request a password reset, you can safely ignore this email &mdash; your account is secure.",

    reg_subject_prefix: "You're In",
    reg_heading: "Registration Confirmed",
    reg_body_tpl: "Your seat is confirmed for",

    wl_subject_prefix: "A Seat Opened Up",
    wl_heading: "You\u{2019}re In",
    wl_body_tpl: "A spot just opened up &mdash; you&rsquo;ve been moved off the waitlist and are now confirmed for",

    soon_subject_suffix: "starts soon!",
    soon_heading: "Starting Soon",
    soon_body_tpl: "is starting in about 15 minutes.",
    soon_ready: "Make sure you&rsquo;re ready to take your seat.",
};

static I18N_FR: I18n = I18n {
    hi: "Bonjour",
    good_luck: "Bonne chance aux tables\u{a0}!",
    footer_tagline: "Gestion de Tournois de Poker",

    pw_subject: "R\u{e9}initialisez votre mot de passe",
    pw_heading: "Mot de passe",
    pw_body: "Nous avons re\u{e7}u une demande de r\u{e9}initialisation de votre mot de passe. Cliquez sur le bouton ci-dessous\u{a0}:",
    pw_cta: "R\u{e9}initialiser",
    pw_disclaimer: "Ce lien expire dans 1 heure. Si vous n&rsquo;avez pas demand\u{e9} de r\u{e9}initialisation, vous pouvez ignorer cet e-mail en toute s\u{e9}curit\u{e9}.",

    reg_subject_prefix: "Inscription confirm\u{e9}e",
    reg_heading: "Inscription Confirm\u{e9}e",
    reg_body_tpl: "Votre place est confirm\u{e9}e pour",

    wl_subject_prefix: "Une place s'est lib\u{e9}r\u{e9}e",
    wl_heading: "Vous \u{ea}tes inscrit\u{a0}!",
    wl_body_tpl: "Une place s&rsquo;est lib\u{e9}r\u{e9}e &mdash; vous avez quitt\u{e9} la liste d&rsquo;attente et \u{ea}tes maintenant confirm\u{e9} pour",

    soon_subject_suffix: "commence bient\u{f4}t\u{a0}!",
    soon_heading: "D\u{e9}but Imminent",
    soon_body_tpl: "commence dans environ 15 minutes.",
    soon_ready: "Assurez-vous d&rsquo;\u{ea}tre pr\u{ea}t\u{a0}!",
};

static I18N_NL: I18n = I18n {
    hi: "Hallo",
    good_luck: "Veel succes aan de tafels!",
    footer_tagline: "Pokertoernooi Management",

    pw_subject: "Stel je wachtwoord opnieuw in",
    pw_heading: "Wachtwoord Resetten",
    pw_body: "We hebben een verzoek ontvangen om je wachtwoord opnieuw in te stellen. Klik op de knop hieronder:",
    pw_cta: "Wachtwoord Resetten",
    pw_disclaimer: "Deze link verloopt over 1 uur. Als je dit niet hebt aangevraagd, kun je deze e-mail veilig negeren.",

    reg_subject_prefix: "Inschrijving bevestigd",
    reg_heading: "Inschrijving Bevestigd",
    reg_body_tpl: "Je plaats is bevestigd voor",

    wl_subject_prefix: "Er is een plek vrijgekomen",
    wl_heading: "Je bent erin!",
    wl_body_tpl: "Er is een plek vrijgekomen &mdash; je bent van de wachtlijst gehaald en nu bevestigd voor",

    soon_subject_suffix: "begint binnenkort!",
    soon_heading: "Begint Binnenkort",
    soon_body_tpl: "begint over ongeveer 15 minuten.",
    soon_ready: "Zorg dat je klaar bent!",
};

// ── Shared HTML layout ──────────────────────────────────────────────

/// Builds the full HTML email wrapped in the branded layout shell.
fn wrap_in_layout(
    heading: &str,
    accent_icon: &str,
    body_html: &str,
    logo_url: &str,
    footer_tagline: &str,
) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="color-scheme" content="dark">
<meta name="supported-color-schemes" content="dark">
<title>{heading}</title>
</head>
<body style="margin:0;padding:0;background-color:#0c0c0e;color:#d4d4d8;font-family:Georgia,'Times New Roman',serif;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;">

<!-- Outer wrapper -->
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background-color:#0c0c0e;">
<tr><td align="center" style="padding:32px 16px 48px;">

  <!-- Gold accent bar -->
  <table role="presentation" width="560" cellpadding="0" cellspacing="0" border="0" style="max-width:560px;">
  <tr><td style="height:3px;background:linear-gradient(90deg,#0c0c0e,#fee78a 20%,#fee78a 80%,#0c0c0e);font-size:0;line-height:0;">&nbsp;</td></tr>
  </table>

  <!-- Main card -->
  <table role="presentation" width="560" cellpadding="0" cellspacing="0" border="0" style="max-width:560px;background-color:#161618;border-left:1px solid #2a2a2e;border-right:1px solid #2a2a2e;border-bottom:1px solid #2a2a2e;">

    <!-- Brand header with logo -->
    <tr><td align="center" style="padding:32px 40px 0;">
      <img src="{logo_url}" alt="PocketPair" width="56" height="56" style="display:block;width:56px;height:56px;border:0;" />
    </td></tr>

    <!-- Decorative divider -->
    <tr><td style="padding:20px 40px 0;">
      <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
      <tr>
        <td style="border-bottom:1px solid #27272a;font-size:0;line-height:0;height:1px;">&nbsp;</td>
      </tr>
      </table>
    </td></tr>

    <!-- Icon + Heading -->
    <tr><td align="center" style="padding:28px 40px 0;">
      <div style="font-size:24px;line-height:1;margin-bottom:10px;color:#fee78a;">{accent_icon}</div>
      <h1 style="margin:0;font-size:26px;font-weight:normal;color:#fee78a;font-family:Georgia,'Times New Roman',serif;letter-spacing:1px;">{heading}</h1>
    </td></tr>

    <!-- Body content -->
    <tr><td style="padding:28px 40px 0;">
      {body_html}
    </td></tr>

    <!-- Suit divider -->
    <tr><td align="center" style="padding:28px 40px 0;">
      <span style="color:#3f3f46;font-size:14px;letter-spacing:8px;">&#9824; &#9829; &#9830; &#9827;</span>
    </td></tr>

    <!-- Footer -->
    <tr><td align="center" style="padding:20px 40px 36px;">
      <p style="margin:0;font-family:Arial,Helvetica,sans-serif;font-size:11px;color:#52524e;line-height:1.6;letter-spacing:0.5px;">
        {footer_tagline}<br>
        &copy; PocketPair
      </p>
    </td></tr>

  </table>

  <!-- Bottom gold accent -->
  <table role="presentation" width="560" cellpadding="0" cellspacing="0" border="0" style="max-width:560px;">
  <tr><td style="height:2px;background:linear-gradient(90deg,#0c0c0e,#fee78a 30%,#fee78a 70%,#0c0c0e);font-size:0;line-height:0;">&nbsp;</td></tr>
  </table>

</td></tr>
</table>

</body>
</html>"##
    )
}

/// Builds a styled CTA button.
fn cta_button(href: &str, label: &str) -> String {
    format!(
        r#"<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="margin:8px auto 4px;">
<tr><td align="center" style="background-color:#fee78a;border-radius:4px;">
  <a href="{href}" target="_blank" style="display:inline-block;padding:14px 40px;font-family:Arial,Helvetica,sans-serif;font-size:14px;font-weight:bold;color:#0c0c0e;text-decoration:none;letter-spacing:1px;text-transform:uppercase;">{label}</a>
</td></tr>
</table>"#
    )
}

/// Builds a body paragraph.
fn paragraph(text: &str) -> String {
    format!(
        r#"<p style="margin:0 0 18px;font-size:16px;line-height:1.7;color:#d4d4d8;">{text}</p>"#
    )
}

/// Builds a muted/secondary paragraph.
fn muted_paragraph(text: &str) -> String {
    format!(
        r#"<p style="margin:0 0 18px;font-family:Arial,Helvetica,sans-serif;font-size:13px;line-height:1.6;color:#71717a;">{text}</p>"#
    )
}

/// Wraps text in a gold bold span.
fn gold(text: &str) -> String {
    format!(r#"<strong style="color:#fee78a;">{text}</strong>"#)
}

// ── EmailService implementation ─────────────────────────────────────

impl EmailService {
    pub fn new(config: EmailConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub fn frontend_base_url(&self) -> &str {
        &self.config.frontend_base_url
    }

    fn logo_url(&self) -> String {
        format!("{}/images/email-logo.png", self.config.frontend_base_url)
    }

    async fn send_email(
        &self,
        to_email: &str,
        to_name: &str,
        subject: &str,
        html: &str,
        text: &str,
    ) -> Result<(), EmailError> {
        let url = format!(
            "https://api.scaleway.com/transactional-email/v1alpha1/regions/{}/emails",
            self.config.scw_region
        );

        let body = json!({
            "from": {
                "email": self.config.sender_email,
                "name": self.config.sender_name,
            },
            "to": [{
                "email": to_email,
                "name": to_name,
            }],
            "subject": subject,
            "html": html,
            "text": text,
            "project_id": self.config.scw_project_id,
        });

        let response = self
            .client
            .post(&url)
            .header("X-Auth-Token", &self.config.scw_secret_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmailError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(EmailError::ApiError { status, body });
        }

        info!("Email sent to {} ({})", to_email, subject);
        Ok(())
    }

    pub async fn send_password_reset(
        &self,
        to_email: &str,
        to_name: &str,
        reset_token: &str,
        locale: Locale,
    ) -> Result<(), EmailError> {
        let t = i18n(locale);
        let reset_link = format!(
            "{}/auth/reset-password?token={}",
            self.config.frontend_base_url, reset_token
        );
        let safe_name = encode_text(to_name);
        let safe_link = encode_text(&reset_link);

        let body_html = format!(
            "{}{}{}{}",
            paragraph(&format!("{} {},", t.hi, safe_name)),
            paragraph(t.pw_body),
            cta_button(&safe_link, t.pw_cta),
            muted_paragraph(t.pw_disclaimer),
        );

        let html = wrap_in_layout(
            t.pw_heading,
            "&#128274;",
            &body_html,
            &self.logo_url(),
            t.footer_tagline,
        );

        let text = format!(
            "{} {},\n\n{}\n\n{}: {}\n\n-- PocketPair",
            t.hi, to_name, t.pw_body, t.pw_cta, reset_link
        );

        self.send_email(to_email, to_name, t.pw_subject, &html, &text)
            .await
    }

    pub async fn send_registration_confirmed(
        &self,
        to_email: &str,
        to_name: &str,
        tournament_name: &str,
        locale: Locale,
    ) -> Result<(), EmailError> {
        let t = i18n(locale);
        let safe_name = encode_text(to_name);
        let safe_tournament = encode_text(tournament_name);
        let subject = format!(
            "{} \u{2660} {}",
            t.reg_subject_prefix, tournament_name
        );

        let body_html = format!(
            "{}{}{}",
            paragraph(&format!("{} {},", t.hi, safe_name)),
            paragraph(&format!(
                "{} {}.",
                t.reg_body_tpl,
                gold(&safe_tournament)
            )),
            paragraph(t.good_luck),
        );

        let html = wrap_in_layout(
            t.reg_heading,
            "&#9824;",
            &body_html,
            &self.logo_url(),
            t.footer_tagline,
        );

        let text = format!(
            "{} {},\n\n{} {}.\n\n{}\n\n-- PocketPair",
            t.hi, to_name, t.reg_body_tpl, tournament_name, t.good_luck
        );

        self.send_email(to_email, to_name, &subject, &html, &text)
            .await
    }

    pub async fn send_waitlist_promoted(
        &self,
        to_email: &str,
        to_name: &str,
        tournament_name: &str,
        locale: Locale,
    ) -> Result<(), EmailError> {
        let t = i18n(locale);
        let safe_name = encode_text(to_name);
        let safe_tournament = encode_text(tournament_name);
        let subject = format!(
            "{} \u{2666} {}",
            t.wl_subject_prefix, tournament_name
        );

        let body_html = format!(
            "{}{}{}",
            paragraph(&format!("{} {},", t.hi, safe_name)),
            paragraph(&format!(
                "{} {}.",
                t.wl_body_tpl,
                gold(&safe_tournament)
            )),
            paragraph(t.good_luck),
        );

        let html = wrap_in_layout(
            t.wl_heading,
            "&#9830;",
            &body_html,
            &self.logo_url(),
            t.footer_tagline,
        );

        let text = format!(
            "{} {},\n\n{} {}.\n\n{}\n\n-- PocketPair",
            t.hi, to_name, t.wl_body_tpl, tournament_name, t.good_luck
        );

        self.send_email(to_email, to_name, &subject, &html, &text)
            .await
    }

    pub async fn send_tournament_starting_soon(
        &self,
        to_email: &str,
        to_name: &str,
        tournament_name: &str,
        locale: Locale,
    ) -> Result<(), EmailError> {
        let t = i18n(locale);
        let safe_name = encode_text(to_name);
        let safe_tournament = encode_text(tournament_name);
        let subject = format!("{} {}", tournament_name, t.soon_subject_suffix);

        let body_html = format!(
            "{}{}{}",
            paragraph(&format!("{} {},", t.hi, safe_name)),
            paragraph(&format!(
                "{} {}",
                gold(&safe_tournament),
                t.soon_body_tpl
            )),
            paragraph(t.soon_ready),
        );

        let html = wrap_in_layout(
            t.soon_heading,
            "&#9829;",
            &body_html,
            &self.logo_url(),
            t.footer_tagline,
        );

        let text = format!(
            "{} {},\n\n{} {}\n\n{}\n\n-- PocketPair",
            t.hi, to_name, tournament_name, t.soon_body_tpl, t.soon_ready
        );

        self.send_email(to_email, to_name, &subject, &html, &text)
            .await
    }
}

// ── Fire-and-forget helper ──────────────────────────────────────────

/// Fire-and-forget email helper. Logs errors but never fails.
pub fn spawn_email(
    email_service: EmailService,
    to_email: String,
    to_name: String,
    email_type: EmailType,
) {
    tokio::spawn(async move {
        let result = match email_type {
            EmailType::RegistrationConfirmed {
                tournament_name,
                locale,
            } => {
                email_service
                    .send_registration_confirmed(&to_email, &to_name, &tournament_name, locale)
                    .await
            }
            EmailType::WaitlistPromoted {
                tournament_name,
                locale,
            } => {
                email_service
                    .send_waitlist_promoted(&to_email, &to_name, &tournament_name, locale)
                    .await
            }
            EmailType::TournamentStartingSoon {
                tournament_name,
                locale,
            } => {
                email_service
                    .send_tournament_starting_soon(&to_email, &to_name, &tournament_name, locale)
                    .await
            }
        };

        if let Err(e) = result {
            warn!("Failed to send email to {}: {}", to_email, e);
        }
    });
}

pub enum EmailType {
    RegistrationConfirmed {
        tournament_name: String,
        locale: Locale,
    },
    WaitlistPromoted {
        tournament_name: String,
        locale: Locale,
    },
    TournamentStartingSoon {
        tournament_name: String,
        locale: Locale,
    },
}
