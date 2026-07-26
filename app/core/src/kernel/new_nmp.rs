//! Production ownership for the supported new-NMP facade.
//!
//! The legacy `NmpApp` remains alive while capabilities migrate one vertical
//! slice at a time. This module owns the new `nmp::Engine` and the lifecycle of
//! capabilities already cut over. Room discovery is the first such capability.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

use nmp::{Engine, EngineConfig, Event, LiveQuery, RelayUrl, Window, WindowContents};
use nmp_next_nip29::group_discovery_demand;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::Cmd;
use crate::kernel::snapshot::DiscoveredRow;

const DISCOVERY_CAP: usize = 256;
const SESSION_STOP_TIMEOUT: Duration = Duration::from_secs(2);

struct DiscoverySession {
    relay_url: String,
    cancel: nmp::ObservationCancel,
    drain: JoinHandle<()>,
}

/// The app-owned new-NMP runtime and its currently active capability handles.
pub(crate) struct NewNmpHandle {
    engine: Engine,
    discovery: Option<DiscoverySession>,
}

impl NewNmpHandle {
    /// Start exactly one Room Explorer discovery observation.
    ///
    /// Re-opening the same view is idempotent. Switching hosts first tears down
    /// the previous observation, so old and new hosts never have simultaneous
    /// ownership of `AppState::discovered_groups`.
    pub(crate) async fn start_discovery(
        &mut self,
        relay_url: String,
        tx: mpsc::UnboundedSender<Cmd>,
    ) {
        if self
            .discovery
            .as_ref()
            .is_some_and(|session| session.relay_url == relay_url)
        {
            return;
        }

        self.stop_discovery().await;

        let host = match RelayUrl::parse(&relay_url) {
            Ok(host) => host,
            Err(error) => {
                tracing::warn!(
                    relay_url,
                    error = %error,
                    "new NMP group discovery rejected an invalid host"
                );
                return;
            }
        };
        let query = LiveQuery(group_discovery_demand(host));
        let window = Window::Expandable {
            initial: NonZeroUsize::new(DISCOVERY_CAP).expect("discovery cap is non-zero"),
            max: NonZeroUsize::new(DISCOVERY_CAP).expect("discovery cap is non-zero"),
        };
        let subscription = match self.engine.observe_async(query, Some(window)) {
            Ok(subscription) => subscription,
            Err(error) => {
                tracing::warn!(
                    relay_url,
                    error = %error,
                    "new NMP group discovery observation failed to open"
                );
                return;
            }
        };
        let cancel = subscription.cancel_handle();
        let projected_host = relay_url.clone();
        let drain = tokio::spawn(async move {
            loop {
                match subscription.next().await {
                    Ok(Some(frame)) => {
                        let Some(window) = frame.window else {
                            tracing::warn!("new NMP group discovery delivered an unwindowed frame");
                            continue;
                        };
                        let rows = discovered_rows_from_window(&window, &projected_host);
                        if tx
                            .send(Cmd::Event(KernelEvent::DiscoveredGroupsUpdated(rows)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(
                            error = ?error,
                            "new NMP group discovery drain rejected concurrent delivery"
                        );
                        break;
                    }
                }
            }
        });

        self.discovery = Some(DiscoverySession {
            relay_url,
            cancel,
            drain,
        });
    }

    /// Cancel the Room Explorer observation and wait for its drain to finish.
    pub(crate) async fn stop_discovery(&mut self) {
        let Some(mut session) = self.discovery.take() else {
            return;
        };
        session.cancel.cancel();
        if tokio::time::timeout(SESSION_STOP_TIMEOUT, &mut session.drain)
            .await
            .is_err()
        {
            tracing::warn!("new NMP group discovery drain did not stop promptly; aborting it");
            session.drain.abort();
            let _ = session.drain.await;
        }
    }

    /// Deterministically stop capability drains before closing the engine.
    pub(crate) async fn shutdown(&mut self) {
        self.stop_discovery().await;
        self.engine.shutdown();
    }
}

impl Drop for NewNmpHandle {
    fn drop(&mut self) {
        if let Some(session) = self.discovery.take() {
            session.cancel.cancel();
            session.drain.abort();
        }
        self.engine.shutdown();
    }
}

/// Start the production new-NMP engine with its own persistent store.
///
/// Failure is non-fatal during the migration: legacy-owned capabilities keep
/// running, while already migrated capabilities fail closed and log the cause.
pub(crate) fn start(data_dir: &str) -> Option<NewNmpHandle> {
    let store_path = store_path(data_dir);
    let Some(parent) = store_path.parent() else {
        tracing::warn!("new NMP store path has no parent directory");
        return None;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        tracing::warn!(
            path = %parent.display(),
            error = %error,
            "failed to create new NMP storage directory"
        );
        return None;
    }

    let config = EngineConfig {
        store_path: Some(store_path.to_string_lossy().into_owned()),
        ..EngineConfig::default()
    };
    match Engine::new(config) {
        Ok(engine) => Some(NewNmpHandle {
            engine,
            discovery: None,
        }),
        Err(error) => {
            tracing::warn!(
                path = %store_path.display(),
                error = %error,
                "failed to start new NMP engine"
            );
            None
        }
    }
}

pub(crate) fn store_path(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir).join("new-nmp").join("engine.redb")
}

fn discovered_rows_from_window(
    window: &WindowContents,
    host_relay_url: &str,
) -> Vec<DiscoveredRow> {
    window
        .rows
        .iter()
        .filter_map(|row| discovered_row_from_event(&row.event, host_relay_url))
        .take(DISCOVERY_CAP)
        .collect()
}

fn discovered_row_from_event(event: &Event, host_relay_url: &str) -> Option<DiscoveredRow> {
    if event.kind.as_u16() != 39_000 {
        return None;
    }

    let group_id = first_tag_value(event, "d")?.to_string();
    if group_id.trim().is_empty() {
        return None;
    }

    Some(DiscoveredRow {
        group_id,
        host_relay_url: host_relay_url.to_string(),
        name: first_tag_value(event, "name").map(str::to_string),
        picture: first_tag_value(event, "picture").map(str::to_string),
        about: first_tag_value(event, "about").map(str::to_string),
        // The supported new-NMP discovery demand intentionally observes only
        // kind:39000 metadata. Kind:39002 membership is a separate migration
        // slice, so inventing a count here would be misleading.
        member_count: 0,
        public: !has_tag(event, "private"),
        open: !has_tag(event, "closed"),
    })
}

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    event.tags.iter().find_map(|tag| {
        let fields = tag.as_slice();
        (fields.first().map(String::as_str) == Some(name))
            .then(|| fields.get(1).map(String::as_str))
            .flatten()
    })
}

fn has_tag(event: &Event, name: &str) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use nmp::{nmp_threads_live, Row, Timestamp};
    use nostr_sdk::{EventBuilder, Keys, Kind, Tag};

    fn metadata_event(group_id: &str, extra_tags: Vec<Vec<&str>>, created_at: u64) -> Event {
        let mut raw_tags = vec![
            vec!["d", group_id],
            vec!["name", "Readers"],
            vec!["picture", "https://example.com/group.png"],
            vec!["about", "Books worth discussing"],
        ];
        raw_tags.extend(extra_tags);
        let tags = raw_tags
            .into_iter()
            .map(|fields| {
                Tag::parse(fields.into_iter().map(str::to_string).collect::<Vec<_>>())
                    .expect("fixture tag must parse")
            })
            .collect::<Vec<_>>();
        EventBuilder::new(Kind::Custom(39_000), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(&Keys::generate())
            .expect("fixture event must sign")
    }

    #[test]
    fn metadata_maps_to_the_existing_discovered_row_contract() {
        let event = metadata_event("books", vec![], 20);
        let row = discovered_row_from_event(&event, "wss://groups.example.com")
            .expect("valid metadata must project");

        assert_eq!(row.group_id, "books");
        assert_eq!(row.host_relay_url, "wss://groups.example.com");
        assert_eq!(row.name.as_deref(), Some("Readers"));
        assert_eq!(
            row.picture.as_deref(),
            Some("https://example.com/group.png")
        );
        assert_eq!(row.about.as_deref(), Some("Books worth discussing"));
        assert_eq!(row.member_count, 0);
        assert!(row.public);
        assert!(row.open);
    }

    #[test]
    fn private_and_closed_markers_are_preserved_without_inventing_membership() {
        let event = metadata_event("invite-only", vec![vec!["private"], vec!["closed"]], 20);
        let row = discovered_row_from_event(&event, "wss://groups.example.com")
            .expect("valid metadata must project");

        assert!(!row.public);
        assert!(!row.open);
        assert_eq!(row.member_count, 0);
    }

    #[test]
    fn malformed_or_wrong_kind_events_are_ignored() {
        let wrong_kind = EventBuilder::new(Kind::Custom(1), "")
            .tags(vec![
                Tag::parse(vec!["d".to_string(), "group".to_string()]).unwrap()
            ])
            .sign_with_keys(&Keys::generate())
            .unwrap();
        let missing_id = EventBuilder::new(Kind::Custom(39_000), "")
            .sign_with_keys(&Keys::generate())
            .unwrap();

        assert!(discovered_row_from_event(&wrong_kind, "wss://groups.example.com").is_none());
        assert!(discovered_row_from_event(&missing_id, "wss://groups.example.com").is_none());
    }

    #[test]
    fn window_projection_keeps_new_nmp_order_and_cap() {
        let rows = (0..=DISCOVERY_CAP)
            .rev()
            .map(|index| {
                let event = metadata_event(&format!("group-{index}"), vec![], index as u64);
                Row {
                    event,
                    sources: BTreeSet::new(),
                }
            })
            .collect();
        let window = WindowContents {
            rows,
            load: nmp::WindowLoad::Idle,
        };

        let projected = discovered_rows_from_window(&window, "wss://groups.example.com");
        assert_eq!(projected.len(), DISCOVERY_CAP);
        assert_eq!(projected[0].group_id, format!("group-{DISCOVERY_CAP}"));
        assert_eq!(projected.last().unwrap().group_id, "group-1");
    }

    #[tokio::test]
    async fn view_session_cancel_and_engine_shutdown_leave_no_worker() {
        let threads_before = nmp_threads_live();
        let data_dir = tempfile::tempdir().unwrap();
        let mut handle = start(data_dir.path().to_str().unwrap()).expect("engine must start");
        let (tx, _rx) = mpsc::unbounded_channel();

        handle
            .start_discovery("ws://127.0.0.1:1".to_string(), tx)
            .await;
        assert!(handle.discovery.is_some());
        handle.stop_discovery().await;
        assert!(handle.discovery.is_none());
        handle.shutdown().await;

        assert_eq!(nmp_threads_live(), threads_before);
        assert!(store_path(data_dir.path().to_str().unwrap()).exists());
    }
}
