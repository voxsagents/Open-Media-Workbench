use std::fs;
use std::path::PathBuf;

use tauri::http::{Method, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext};

use opentake_core::media::error::MediaEngineError;
use opentake_core::media::range::{
    determine_mime_type, parse_byte_range, read_range_slice, ResolvedByteRange,
    MAX_RANGE_CHUNK_BYTES,
};

use crate::commands::media_import::MediaLibraryState;
use crate::commands::playback::PlaybackState;

/// Handles incoming `opentake-media://` URI scheme protocol requests.
///
/// # Security Invariants
/// 1. Only accepts opaque asset IDs registered in the active `MediaLibrarySession`.
/// 2. Does NOT accept arbitrary filesystem paths in the URI.
/// 3. Validates source file existence on the host filesystem before serving.
/// 4. Restricts HTTP methods to GET and HEAD.
/// 5. Strictly streams bounded byte slices (max 2 MiB) to prevent memory exhaustion.
pub fn handle_media_protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    req: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    // 1. Validate HTTP Method
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header("Allow", "GET, HEAD")
            .body(b"Method Not Allowed".to_vec())
            .unwrap();
    }

    let is_head = req.method() == Method::HEAD;

    // 2. Parse URI to extract Asset ID and Query Parameters
    let uri = req.uri();
    let raw_path = uri.path().trim_start_matches('/');
    let (asset_id, _) = raw_path.split_once('/').unwrap_or((raw_path, ""));

    // Reject empty or non-alphanumeric/hyphen/underscore asset IDs (traversal prevention)
    if asset_id.is_empty()
        || !asset_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/plain")
            .body(b"Invalid media asset ID".to_vec())
            .unwrap();
    }

    let is_proxy_requested = uri
        .query()
        .map(|q| q.contains("proxy=1") || q.contains("proxy=true"))
        .unwrap_or(false);

    // 3. Resolve MediaAsset from MediaLibrarySession
    let app = ctx.app_handle();
    let library_state = app.state::<MediaLibraryState>();

    let asset_opt = tauri::async_runtime::block_on(async {
        library_state.session.get_asset_by_id(asset_id).await
    });

    let (file_to_serve, mime_type) = if let Some(asset) = asset_opt {
        let file_path = if is_proxy_requested {
            let playback_state = app.state::<PlaybackState>();
            let cached_proxy = tauri::async_runtime::block_on(async {
                playback_state.proxy_engine.find_cached_proxy(&asset).await
            });

            match cached_proxy {
                Some(mapping) => mapping.proxy_path,
                None => {
                    return Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .header("Content-Type", "text/plain")
                        .body(b"Preview proxy not available for this asset".to_vec())
                        .unwrap();
                }
            }
        } else {
            PathBuf::from(&asset.source.canonical_path)
        };
        let format_hint = Some(asset.metadata.container.format_name.as_str());
        let mime = determine_mime_type(&file_path, format_hint);
        (file_path, mime)
    } else {
        // Fallback: check if asset_id is an active dubbing audio asset
        let dubbing_file =
            if let Some(dub_state) = app.try_state::<crate::commands::dubbing::DubbingState>() {
                tauri::async_runtime::block_on(async {
                    let guard = dub_state.active_project.read().await;
                    guard
                        .as_ref()
                        .and_then(|p| p.assets.get(asset_id).map(|a| a.file_path.clone()))
                })
            } else {
                None
            };

        if let Some(file_path) = dubbing_file {
            let mime = determine_mime_type(&file_path, Some("wav"));
            (file_path, mime)
        } else {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(b"Media asset not found or authorization revoked".to_vec())
                .unwrap();
        }
    };

    // 5. Verify file on disk
    if !file_to_serve.is_file() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/plain")
            .body(b"Target media file does not exist on disk".to_vec())
            .unwrap();
    }

    let Ok(metadata) = fs::metadata(&file_to_serve) else {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "text/plain")
            .body(b"Failed to read media file metadata".to_vec())
            .unwrap();
    };

    let file_size = metadata.len();

    // 6. Handle Range Request
    let range_header = req
        .headers()
        .get("Range")
        .or_else(|| req.headers().get("range"))
        .and_then(|h| h.to_str().ok());

    match parse_byte_range(range_header, file_size) {
        Err(MediaEngineError::RangeNotSatisfiable { .. }) => Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header("Content-Range", format!("bytes */{file_size}"))
            .header("Accept-Ranges", "bytes")
            .header("Content-Type", "text/plain")
            .body(b"Range Not Satisfiable".to_vec())
            .unwrap(),

        Err(MediaEngineError::InvalidRange { details }) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "text/plain")
            .body(format!("Invalid Range header: {details}").into_bytes())
            .unwrap(),

        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "text/plain")
            .body(b"Internal range error".to_vec())
            .unwrap(),

        // Single valid byte-range request -> HTTP 206 Partial Content
        Ok(Some(resolved_range)) => {
            if is_head {
                Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header("Content-Type", mime_type)
                    .header("Content-Range", resolved_range.content_range_header())
                    .header("Content-Length", resolved_range.length().to_string())
                    .header("Accept-Ranges", "bytes")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Vec::new())
                    .unwrap()
            } else {
                match read_range_slice(&file_to_serve, &resolved_range, MAX_RANGE_CHUNK_BYTES) {
                    Ok(bytes) => {
                        let actual_end = resolved_range.start + bytes.len() as u64 - 1;
                        let content_range = format!(
                            "bytes {}-{}/{}",
                            resolved_range.start, actual_end, file_size
                        );

                        Response::builder()
                            .status(StatusCode::PARTIAL_CONTENT)
                            .header("Content-Type", mime_type)
                            .header("Content-Range", content_range)
                            .header("Content-Length", bytes.len().to_string())
                            .header("Accept-Ranges", "bytes")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(bytes)
                            .unwrap()
                    }
                    Err(e) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/plain")
                        .body(e.user_message().into_bytes())
                        .unwrap(),
                }
            }
        }

        // Full file request (No Range header provided)
        Ok(None) => {
            if is_head {
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Length", file_size.to_string())
                    .header("Accept-Ranges", "bytes")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Vec::new())
                    .unwrap()
            } else if file_size <= MAX_RANGE_CHUNK_BYTES as u64 {
                // Small file: deliver full content with HTTP 200
                let full_range = ResolvedByteRange {
                    start: 0,
                    end: file_size.saturating_sub(1),
                    total_size: file_size,
                };
                match read_range_slice(&file_to_serve, &full_range, MAX_RANGE_CHUNK_BYTES) {
                    Ok(bytes) => Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", mime_type)
                        .header("Content-Length", bytes.len().to_string())
                        .header("Accept-Ranges", "bytes")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(bytes)
                        .unwrap(),
                    Err(e) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/plain")
                        .body(e.user_message().into_bytes())
                        .unwrap(),
                }
            } else {
                // Large file without Range: deliver first chunk with HTTP 206 to prevent large heap allocations
                let first_range = ResolvedByteRange {
                    start: 0,
                    end: (MAX_RANGE_CHUNK_BYTES as u64).min(file_size) - 1,
                    total_size: file_size,
                };
                match read_range_slice(&file_to_serve, &first_range, MAX_RANGE_CHUNK_BYTES) {
                    Ok(bytes) => {
                        let actual_end = bytes.len() as u64 - 1;
                        let content_range = format!("bytes 0-{}/{}", actual_end, file_size);

                        Response::builder()
                            .status(StatusCode::PARTIAL_CONTENT)
                            .header("Content-Type", mime_type)
                            .header("Content-Range", content_range)
                            .header("Content-Length", bytes.len().to_string())
                            .header("Accept-Ranges", "bytes")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(bytes)
                            .unwrap()
                    }
                    Err(e) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/plain")
                        .body(e.user_message().into_bytes())
                        .unwrap(),
                }
            }
        }
    }
}
