//! Read the configured Fleet owner without holding a Tracker transaction.
use domain::sdlc_native_admission::NativeAdmission;
use domain::sdlc_pm_draft::PmDraftReservation;
use reqwest::{Client, Url, header};
use shared::AppError;
use std::time::Duration;

#[derive(Clone)]
pub struct FleetNativeReader {
    origin: Url,
    authorization: header::HeaderValue,
    client: Client,
}

fn unavailable() -> AppError {
    AppError::Unavailable("Fleet native admission observation is unavailable".into())
}

impl FleetNativeReader {
    pub fn new(origin: &str, token: &str) -> Result<Self, AppError> {
        let origin = Url::parse(origin).map_err(|_| unavailable())?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !(32..=512).contains(&token.len())
            || !token.bytes().all(|v| v.is_ascii_graphic())
        {
            return Err(unavailable());
        }
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| unavailable())?;
        authorization.set_sensitive(true);
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            origin,
            authorization,
            client,
        })
    }

    pub async fn read(
        &self,
        reservation: &PmDraftReservation,
    ) -> Result<NativeAdmission, AppError> {
        let mut url = self.origin.clone();
        url.set_path(&format!(
            "/internal/runtime/v1/pm/executions/{}/admission",
            reservation.assignment.execution_id
        ));
        let mut response = self
            .client
            .get(url)
            .header(header::AUTHORIZATION, self.authorization.clone())
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|_| unavailable())?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len() + chunk.len() > 16384 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let observed: NativeAdmission =
            serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        observed.validate(reservation)?;
        Ok(observed)
    }
}
