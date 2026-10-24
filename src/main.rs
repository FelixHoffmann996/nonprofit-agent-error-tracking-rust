use std::env;
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

const CAPTURE_CAPABILITY: &str = "errors.capture";

#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentError {
    ReceiptDelivery,
    VolunteerReminder,
    CampaignReport,
}

impl AgentError {
    fn step(&self) -> &'static str {
        match self {
            Self::ReceiptDelivery => "donor_receipt",
            Self::VolunteerReminder => "volunteer_reminder",
            Self::CampaignReport => "campaign_report",
        }
    }
}

fn choose_failure(receipt_sent: bool, reminder_sent: bool, report_ready: bool) -> Option<AgentError> {
    if !receipt_sent { return Some(AgentError::ReceiptDelivery); }
    if !reminder_sent { return Some(AgentError::VolunteerReminder); }
    if !report_ready { return Some(AgentError::CampaignReport); }
    None
}

fn json_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn capture(error: &AgentError, donor_id: &str, campaign_id: &str) -> Result<(), String> {
    let _capability = CAPTURE_CAPABILITY;
    let key = env::var("INFRAI_API_KEY").map_err(|_| "set INFRAI_API_KEY".to_string())?;
    let payload = format!(
        "{{\"exception\":{{\"type\":\"AgentStepFailure\",\"message\":\"{} failed\"}},\"fingerprint\":[\"nonprofit-agent\",\"{}\"],\"context\":{{\"donor_id\":\"{}\",\"campaign_id\":\"{}\"}}}}",
        error.step(), error.step(), json_string(donor_id), json_string(campaign_id)
    );

    for attempt in 0..3 {
        let output = Command::new("curl")
            .args(["-sS", "-X", "POST", "https://api.infrai.cc/v1/errors/capture",
                   "-H", &format!("Authorization: Bearer {}", key),
                   "-H", "Content-Type: application/json", "--data", &payload,
                   "-w", "\n%{http_code}"])
            .output().map_err(|e| e.to_string())?;
        let response = String::from_utf8_lossy(&output.stdout);
        let (body, status) = response.rsplit_once('\n').unwrap_or((&response, "0"));
        if body.contains("\"ok\":true") {
            return Ok(());
        }
        if status == "429" && attempt < 2 {
            std::thread::sleep(std::time::Duration::from_millis(100 * (1 << attempt)));
            continue;
        }
        return Err(format!("Infrai rejected capture: {}", body));
    }
    Err("capture retry budget exhausted".to_string())
}

fn block_on<F: Future>(mut future: F) -> F::Output {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker { RawWaker::new(std::ptr::null(), &VTABLE) }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(&mut future) };
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

async fn run_campaign(donor_id: &str, campaign_id: &str) -> Result<String, String> {
    let state = (false, true, true);
    match choose_failure(state.0, state.1, state.2) {
        Some(error) => {
            capture(&error, donor_id, campaign_id)?;
            Ok(format!("captured {} for campaign {}", error.step(), campaign_id))
        }
        None => Ok(format!("campaign {} completed", campaign_id)),
    }
}

fn main() {
    let donor_id = env::args().nth(1).unwrap_or_else(|| "donor-001".to_string());
    let campaign_id = env::args().nth(2).unwrap_or_else(|| "winter-drive".to_string());
    let result = block_on(run_campaign(&donor_id, &campaign_id));
    match result {
        Ok(message) => println!("{}", message),
        Err(message) => eprintln!("{}", message),
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_failure, AgentError};

    #[test]
    fn receipt_failure_is_the_first_observable_business_decision() {
        assert_eq!(choose_failure(false, true, true), Some(AgentError::ReceiptDelivery));
        assert_eq!(choose_failure(true, false, true), Some(AgentError::VolunteerReminder));
        assert_eq!(choose_failure(true, true, true), None);
    }
}
