use crate::{
    library::{MediaMetadata, MetadataResolver, MetadataStorage},
    media_enrichment::xtream::{
        apply_artwork_patch_to_series, apply_artwork_patch_to_video, series_fact_patch_from_metadata,
        video_fact_patch_from_metadata,
    },
    model::AppConfig,
};
use log::debug;
use shared::model::{PlaylistGroup, PlaylistItem, PlaylistItemType, StreamProperties};
use std::path::PathBuf;

pub async fn enrich_media_server_playlist_with_tmdb_artwork(
    app_config: &AppConfig,
    client: &reqwest::Client,
    playlist: &mut [PlaylistGroup],
) -> usize {
    let Some(resolver) = media_server_tmdb_artwork_resolver(app_config, client) else {
        return 0;
    };

    let mut changed = 0;
    for group in playlist {
        for item in &mut group.channels {
            let Some((title, tmdb_id, is_movie)) = tmdb_artwork_lookup_candidate(item) else {
                continue;
            };
            let Some(metadata) = resolver.resolve_from_title(&title, Some(tmdb_id), is_movie, true).await else {
                continue;
            };
            if apply_tmdb_artwork_metadata_to_media_server_playlist_item(item, &metadata) {
                changed += 1;
            }
        }
    }

    if changed > 0 {
        debug!("Applied TMDB artwork enrichment to {changed} media-server playlist items");
    }
    changed
}

pub fn apply_tmdb_artwork_metadata_to_media_server_playlist_item(
    item: &mut PlaylistItem,
    metadata: &MediaMetadata,
) -> bool {
    let mut header_logo_patch: Option<String> = None;
    let mut changed = match item.header.additional_properties.as_mut() {
        Some(StreamProperties::Video(video)) => {
            let patch = video_fact_patch_from_metadata(video, metadata);
            header_logo_patch.clone_from(&patch.artwork.poster_url);
            apply_artwork_patch_to_video(video, &patch)
        }
        Some(StreamProperties::Series(series)) if item.header.item_type == PlaylistItemType::SeriesInfo => {
            let patch = series_fact_patch_from_metadata(series, metadata);
            header_logo_patch.clone_from(&patch.artwork.poster_url);
            apply_artwork_patch_to_series(series, &patch)
        }
        _ => false,
    };

    if let Some(poster_url) = header_logo_patch.as_deref() {
        changed |= set_missing_header_logo(item, poster_url);
    }

    changed
}

fn set_missing_header_logo(item: &mut PlaylistItem, poster_url: &str) -> bool {
    if item.header.logo.trim().is_empty() {
        item.header.logo = poster_url.into();
        return true;
    }
    false
}

fn media_server_tmdb_artwork_resolver(app_config: &AppConfig, client: &reqwest::Client) -> Option<MetadataResolver> {
    let config = app_config.config.load();
    let metadata_update_config = config.metadata_update.as_ref()?;
    if !metadata_update_config.tmdb.enabled || !metadata_update_config.tmdb.artwork {
        return None;
    }

    let tmdb_storage = Some(MetadataStorage::new(PathBuf::from(&metadata_update_config.cache_path)));
    Some(MetadataResolver::from_config(None, Some(metadata_update_config), client.clone(), tmdb_storage))
}

fn tmdb_artwork_lookup_candidate(item: &PlaylistItem) -> Option<(String, u32, bool)> {
    match item.header.additional_properties.as_ref()? {
        StreamProperties::Video(video) => {
            let title = non_empty_title(video.name.as_ref()).or_else(|| non_empty_title(item.header.title.as_ref()))?;
            Some((title, video.tmdb?, true))
        }
        StreamProperties::Series(series) if item.header.item_type == PlaylistItemType::SeriesInfo => {
            let title = non_empty_title(series.name.as_ref()).or_else(|| non_empty_title(item.header.title.as_ref()))?;
            Some((title, series.tmdb?, false))
        }
        _ => None,
    }
}

fn non_empty_title(title: &str) -> Option<String> {
    let title = title.trim();
    (!title.is_empty()).then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{MovieMetadata, SeriesMetadata};
    use shared::{
        model::{PlaylistItemHeader, SeriesStreamProperties, VideoStreamProperties, XtreamCluster},
        utils::Internable,
    };

    #[test]
    fn applies_tmdb_movie_artwork_to_media_server_playlist_item() {
        let mut item = PlaylistItem {
            header: PlaylistItemHeader {
                title: "The Matrix".intern(),
                xtream_cluster: XtreamCluster::Video,
                item_type: PlaylistItemType::Video,
                additional_properties: Some(StreamProperties::Video(Box::new(VideoStreamProperties {
                    name: "The Matrix".intern(),
                    tmdb: Some(603),
                    ..VideoStreamProperties::default()
                }))),
                ..PlaylistItemHeader::default()
            },
        };
        let metadata = MediaMetadata::Movie(MovieMetadata {
            tmdb_id: Some(603),
            poster: Some("https://image.example/movie-poster.jpg".to_string()),
            fanart: Some("https://image.example/movie-backdrop.jpg".to_string()),
            ..MovieMetadata::default()
        });

        assert!(apply_tmdb_artwork_metadata_to_media_server_playlist_item(&mut item, &metadata));

        let Some(StreamProperties::Video(video)) = item.header.additional_properties.as_ref() else {
            panic!("expected video properties");
        };
        assert_eq!(video.stream_icon.as_ref(), "https://image.example/movie-poster.jpg");
        assert_eq!(item.header.logo.as_ref(), "https://image.example/movie-poster.jpg");
        let details = video.details.as_ref().expect("details are created for movie artwork");
        assert_eq!(details.cover_big.as_deref(), Some("https://image.example/movie-poster.jpg"));
        assert_eq!(details.movie_image.as_deref(), Some("https://image.example/movie-poster.jpg"));
        assert_eq!(
            details.backdrop_path.as_ref().and_then(|values| values.first()).map(std::sync::Arc::as_ref),
            Some("https://image.example/movie-backdrop.jpg")
        );
    }

    #[test]
    fn applies_tmdb_series_artwork_to_media_server_playlist_item() {
        let mut item = PlaylistItem {
            header: PlaylistItemHeader {
                title: "Breaking Bad".intern(),
                xtream_cluster: XtreamCluster::Series,
                item_type: PlaylistItemType::SeriesInfo,
                additional_properties: Some(StreamProperties::Series(Box::new(SeriesStreamProperties {
                    name: "Breaking Bad".intern(),
                    tmdb: Some(1396),
                    ..SeriesStreamProperties::default()
                }))),
                ..PlaylistItemHeader::default()
            },
        };
        let metadata = MediaMetadata::Series(SeriesMetadata {
            tmdb_id: Some(1396),
            poster: Some("https://image.example/series-poster.jpg".to_string()),
            fanart: Some("https://image.example/series-backdrop.jpg".to_string()),
            ..SeriesMetadata::default()
        });

        assert!(apply_tmdb_artwork_metadata_to_media_server_playlist_item(&mut item, &metadata));

        let Some(StreamProperties::Series(series)) = item.header.additional_properties.as_ref() else {
            panic!("expected series properties");
        };
        assert_eq!(series.cover.as_ref(), "https://image.example/series-poster.jpg");
        assert_eq!(item.header.logo.as_ref(), "https://image.example/series-poster.jpg");
        assert_eq!(
            series.backdrop_path.as_ref().and_then(|values| values.first()).map(std::sync::Arc::as_ref),
            Some("https://image.example/series-backdrop.jpg")
        );
    }

    #[test]
    fn skips_media_server_artwork_when_tmdb_id_is_missing() {
        let item = PlaylistItem {
            header: PlaylistItemHeader {
                title: "The Matrix".intern(),
                item_type: PlaylistItemType::Video,
                additional_properties: Some(StreamProperties::Video(Box::new(VideoStreamProperties {
                    name: "The Matrix".intern(),
                    ..VideoStreamProperties::default()
                }))),
                ..PlaylistItemHeader::default()
            },
        };

        assert!(tmdb_artwork_lookup_candidate(&item).is_none());
    }
}
