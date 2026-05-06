use crate::{
    library::MediaMetadata,
    media_enrichment::{
        facts::{build_missing_fact_patch, MediaArtworkFacts, MediaFactPatch, MediaItemFacts, MediaItemKind},
        parsed_title::supplied_release_year_from_title,
        tmdb::supplied_facts_from_metadata,
    },
};
use shared::model::{SeriesStreamProperties, VideoStreamDetailProperties, VideoStreamProperties};
use std::sync::Arc;

pub fn video_fact_patch_from_metadata(
    properties: &VideoStreamProperties,
    metadata: &MediaMetadata,
) -> MediaFactPatch {
    build_missing_fact_patch(&video_current_facts(properties), &supplied_facts_from_metadata(metadata))
}

pub fn video_fact_patch_from_title(
    properties: &VideoStreamProperties,
    title: &str,
) -> Option<(u32, MediaFactPatch)> {
    let (year, supplied) = supplied_release_year_from_title(MediaItemKind::Movie, title)?;
    Some((year, build_missing_fact_patch(&video_current_facts(properties), &supplied)))
}

pub fn series_fact_patch_from_metadata(
    properties: &SeriesStreamProperties,
    metadata: &MediaMetadata,
) -> MediaFactPatch {
    build_missing_fact_patch(&series_current_facts(properties), &supplied_facts_from_metadata(metadata))
}

pub fn series_fact_patch_from_title(
    properties: &SeriesStreamProperties,
    title: &str,
) -> Option<(u32, MediaFactPatch)> {
    let (year, supplied) = supplied_release_year_from_title(MediaItemKind::Series, title)?;
    Some((year, build_missing_fact_patch(&series_current_facts(properties), &supplied)))
}

pub fn apply_fact_patch_to_video(properties: &mut VideoStreamProperties, patch: &MediaFactPatch) -> bool {
    let mut changed = false;

    if properties.tmdb.is_none() {
        if let Some(tmdb_id) = patch.tmdb_id {
            properties.tmdb = Some(tmdb_id);
            changed = true;
        }
    }

    if let Some(release_date) = patch.release_date.as_deref() {
        if properties.details.is_none() {
            properties.details = Some(VideoStreamDetailProperties::default());
        }
        if let Some(details) = properties.details.as_mut() {
            if details.release_date.is_none() {
                details.release_date = Some(Arc::<str>::from(release_date));
                changed = true;
            }
        }
    }

    changed
}

pub fn apply_fact_patch_to_series(properties: &mut SeriesStreamProperties, patch: &MediaFactPatch) -> bool {
    let mut changed = false;

    if properties.tmdb.is_none() {
        if let Some(tmdb_id) = patch.tmdb_id {
            properties.tmdb = Some(tmdb_id);
            changed = true;
        }
    }

    if properties.release_date.is_none() {
        if let Some(release_date) = patch.release_date.as_deref() {
            properties.release_date = Some(Arc::<str>::from(release_date));
            changed = true;
        }
    }

    changed
}

pub fn apply_artwork_patch_to_video(properties: &mut VideoStreamProperties, patch: &MediaFactPatch) -> bool {
    let mut changed = false;

    if let Some(poster_url) = patch.artwork.poster_url.as_deref() {
        if properties.stream_icon.trim().is_empty() {
            properties.stream_icon = Arc::<str>::from(poster_url);
            changed = true;
        }

        if properties.details.is_none() {
            properties.details = Some(VideoStreamDetailProperties::default());
        }
        if let Some(details) = properties.details.as_mut() {
            changed |= set_missing_arc_field(&mut details.cover_big, poster_url);
            changed |= set_missing_arc_field(&mut details.movie_image, poster_url);
        }
    }

    if let Some(backdrop_url) = patch.artwork.backdrop_url.as_deref() {
        if properties.details.is_none() {
            properties.details = Some(VideoStreamDetailProperties::default());
        }
        if let Some(details) = properties.details.as_mut() {
            if details.backdrop_path.as_ref().is_none_or(|backdrops| backdrops.iter().all(|url| url.trim().is_empty())) {
                details.backdrop_path = Some(vec![Arc::<str>::from(backdrop_url)]);
                changed = true;
            }
        }
    }

    changed
}

pub fn apply_artwork_patch_to_series(properties: &mut SeriesStreamProperties, patch: &MediaFactPatch) -> bool {
    let mut changed = false;

    if let Some(poster_url) = patch.artwork.poster_url.as_deref() {
        if properties.cover.trim().is_empty() {
            properties.cover = Arc::<str>::from(poster_url);
            changed = true;
        }
    }

    if let Some(backdrop_url) = patch.artwork.backdrop_url.as_deref() {
        if properties.backdrop_path.as_ref().is_none_or(|backdrops| backdrops.iter().all(|url| url.trim().is_empty())) {
            properties.backdrop_path = Some(vec![Arc::<str>::from(backdrop_url)]);
            changed = true;
        }
    }

    changed
}

fn video_current_facts(properties: &VideoStreamProperties) -> MediaItemFacts {
    MediaItemFacts::movie(
        properties.tmdb,
        properties.details.as_ref().and_then(|details| details.release_date.as_ref()).map(ToString::to_string),
    )
    .with_artwork(video_current_artwork(properties))
}

fn series_current_facts(properties: &SeriesStreamProperties) -> MediaItemFacts {
    MediaItemFacts::series(properties.tmdb, properties.release_date.as_ref().map(ToString::to_string))
        .with_artwork(series_current_artwork(properties))
}

fn video_current_artwork(properties: &VideoStreamProperties) -> MediaArtworkFacts {
    let details = properties.details.as_ref();
    MediaArtworkFacts::new(
        non_empty_string(properties.stream_icon.as_ref())
            .or_else(|| details.and_then(|details| details.cover_big.as_deref()).and_then(non_empty_string))
            .or_else(|| details.and_then(|details| details.movie_image.as_deref()).and_then(non_empty_string)),
        details
            .and_then(|details| details.backdrop_path.as_ref())
            .and_then(|backdrops| backdrops.iter().find_map(|url| non_empty_string(url.as_ref()))),
    )
}

fn series_current_artwork(properties: &SeriesStreamProperties) -> MediaArtworkFacts {
    MediaArtworkFacts::new(
        non_empty_string(properties.cover.as_ref()),
        properties
            .backdrop_path
            .as_ref()
            .and_then(|backdrops| backdrops.iter().find_map(|url| non_empty_string(url.as_ref()))),
    )
}

fn set_missing_arc_field(field: &mut Option<Arc<str>>, value: &str) -> bool {
    if field.as_ref().is_some_and(|existing| !existing.trim().is_empty()) {
        return false;
    }
    *field = Some(Arc::<str>::from(value));
    true
}

fn non_empty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_video_patch_without_overwriting_existing_facts() {
        let mut properties = VideoStreamProperties {
            tmdb: Some(603),
            details: Some(VideoStreamDetailProperties {
                release_date: Some("1999-03-31".into()),
                ..VideoStreamDetailProperties::default()
            }),
            ..VideoStreamProperties::default()
        };
        let patch = MediaFactPatch {
            tmdb_id: Some(999),
            release_date: Some("2000-01-01".to_string()),
            ..MediaFactPatch::default()
        };

        assert!(!apply_fact_patch_to_video(&mut properties, &patch));
        assert_eq!(properties.tmdb, Some(603));
        assert_eq!(
            properties.details.as_ref().and_then(|details| details.release_date.as_deref()),
            Some("1999-03-31")
        );
    }

    #[test]
    fn builds_video_patch_from_parseable_title() {
        let properties = VideoStreamProperties::default();
        let Some((year, patch)) = video_fact_patch_from_title(&properties, "The Matrix 1999") else {
            panic!("expected title year patch");
        };

        assert_eq!(year, 1999);
        assert_eq!(patch.release_date.as_deref(), Some("1999-01-01"));
    }

    #[test]
    fn builds_video_artwork_patch_from_metadata() {
        let properties = VideoStreamProperties::default();
        let metadata = MediaMetadata::Movie(crate::library::MovieMetadata {
            poster: Some("https://image.example/poster.jpg".to_string()),
            fanart: Some("https://image.example/backdrop.jpg".to_string()),
            ..crate::library::MovieMetadata::default()
        });

        let patch = video_fact_patch_from_metadata(&properties, &metadata);

        assert_eq!(patch.artwork.poster_url.as_deref(), Some("https://image.example/poster.jpg"));
        assert_eq!(patch.artwork.backdrop_url.as_deref(), Some("https://image.example/backdrop.jpg"));
    }

    #[test]
    fn applies_video_artwork_patch_to_missing_fields() {
        let mut properties = VideoStreamProperties::default();
        let patch = MediaFactPatch {
            artwork: MediaArtworkFacts::from_urls(
                Some("https://image.example/poster.jpg"),
                Some("https://image.example/backdrop.jpg"),
            ),
            ..MediaFactPatch::default()
        };

        assert!(apply_artwork_patch_to_video(&mut properties, &patch));
        assert_eq!(properties.stream_icon.as_ref(), "https://image.example/poster.jpg");
        let details = properties.details.as_ref().expect("details are created for artwork");
        assert_eq!(details.cover_big.as_deref(), Some("https://image.example/poster.jpg"));
        assert_eq!(details.movie_image.as_deref(), Some("https://image.example/poster.jpg"));
        assert_eq!(details.backdrop_path.as_ref().and_then(|values| values.first()).map(Arc::as_ref), Some("https://image.example/backdrop.jpg"));
    }

    #[test]
    fn applies_video_artwork_patch_without_overwriting_existing_artwork() {
        let mut properties = VideoStreamProperties {
            stream_icon: "https://image.example/current-poster.jpg".into(),
            details: Some(VideoStreamDetailProperties {
                cover_big: Some("https://image.example/current-cover.jpg".into()),
                movie_image: Some("https://image.example/current-image.jpg".into()),
                backdrop_path: Some(vec!["https://image.example/current-backdrop.jpg".into()]),
                ..VideoStreamDetailProperties::default()
            }),
            ..VideoStreamProperties::default()
        };
        let patch = MediaFactPatch {
            artwork: MediaArtworkFacts::from_urls(
                Some("https://image.example/new-poster.jpg"),
                Some("https://image.example/new-backdrop.jpg"),
            ),
            ..MediaFactPatch::default()
        };

        assert!(!apply_artwork_patch_to_video(&mut properties, &patch));
        assert_eq!(properties.stream_icon.as_ref(), "https://image.example/current-poster.jpg");
        let details = properties.details.as_ref().expect("details remain present");
        assert_eq!(details.cover_big.as_deref(), Some("https://image.example/current-cover.jpg"));
        assert_eq!(details.movie_image.as_deref(), Some("https://image.example/current-image.jpg"));
        assert_eq!(details.backdrop_path.as_ref().and_then(|values| values.first()).map(Arc::as_ref), Some("https://image.example/current-backdrop.jpg"));
    }

    #[test]
    fn applies_video_patch_to_missing_facts() {
        let mut properties = VideoStreamProperties::default();
        let patch = MediaFactPatch {
            tmdb_id: Some(603),
            release_date: Some("1999-01-01".to_string()),
            ..MediaFactPatch::default()
        };

        assert!(apply_fact_patch_to_video(&mut properties, &patch));
        assert_eq!(properties.tmdb, Some(603));
        assert_eq!(
            properties.details.as_ref().and_then(|details| details.release_date.as_deref()),
            Some("1999-01-01")
        );
    }

    #[test]
    fn applies_series_patch_without_overwriting_existing_facts() {
        let mut properties = SeriesStreamProperties {
            tmdb: Some(1396),
            release_date: Some("2008-01-20".into()),
            ..SeriesStreamProperties::default()
        };
        let patch = MediaFactPatch {
            tmdb_id: Some(999),
            release_date: Some("2009-01-01".to_string()),
            ..MediaFactPatch::default()
        };

        assert!(!apply_fact_patch_to_series(&mut properties, &patch));
        assert_eq!(properties.tmdb, Some(1396));
        assert_eq!(properties.release_date.as_deref(), Some("2008-01-20"));
    }

    #[test]
    fn builds_series_patch_from_parseable_title() {
        let properties = SeriesStreamProperties::default();
        let Some((year, patch)) = series_fact_patch_from_title(&properties, "Breaking Bad 2008") else {
            panic!("expected title year patch");
        };

        assert_eq!(year, 2008);
        assert_eq!(patch.release_date.as_deref(), Some("2008-01-01"));
    }

    #[test]
    fn builds_series_artwork_patch_from_metadata() {
        let properties = SeriesStreamProperties::default();
        let metadata = MediaMetadata::Series(crate::library::SeriesMetadata {
            poster: Some("https://image.example/series-poster.jpg".to_string()),
            fanart: Some("https://image.example/series-backdrop.jpg".to_string()),
            ..crate::library::SeriesMetadata::default()
        });

        let patch = series_fact_patch_from_metadata(&properties, &metadata);

        assert_eq!(patch.artwork.poster_url.as_deref(), Some("https://image.example/series-poster.jpg"));
        assert_eq!(patch.artwork.backdrop_url.as_deref(), Some("https://image.example/series-backdrop.jpg"));
    }

    #[test]
    fn applies_series_artwork_patch_to_missing_fields() {
        let mut properties = SeriesStreamProperties::default();
        let patch = MediaFactPatch {
            artwork: MediaArtworkFacts::from_urls(
                Some("https://image.example/series-poster.jpg"),
                Some("https://image.example/series-backdrop.jpg"),
            ),
            ..MediaFactPatch::default()
        };

        assert!(apply_artwork_patch_to_series(&mut properties, &patch));
        assert_eq!(properties.cover.as_ref(), "https://image.example/series-poster.jpg");
        assert_eq!(properties.backdrop_path.as_ref().and_then(|values| values.first()).map(Arc::as_ref), Some("https://image.example/series-backdrop.jpg"));
    }

    #[test]
    fn applies_series_artwork_patch_without_overwriting_existing_artwork() {
        let mut properties = SeriesStreamProperties {
            cover: "https://image.example/current-series-poster.jpg".into(),
            backdrop_path: Some(vec!["https://image.example/current-series-backdrop.jpg".into()]),
            ..SeriesStreamProperties::default()
        };
        let patch = MediaFactPatch {
            artwork: MediaArtworkFacts::from_urls(
                Some("https://image.example/new-series-poster.jpg"),
                Some("https://image.example/new-series-backdrop.jpg"),
            ),
            ..MediaFactPatch::default()
        };

        assert!(!apply_artwork_patch_to_series(&mut properties, &patch));
        assert_eq!(properties.cover.as_ref(), "https://image.example/current-series-poster.jpg");
        assert_eq!(properties.backdrop_path.as_ref().and_then(|values| values.first()).map(Arc::as_ref), Some("https://image.example/current-series-backdrop.jpg"));
    }

    #[test]
    fn applies_series_patch_to_missing_facts() {
        let mut properties = SeriesStreamProperties::default();
        let patch = MediaFactPatch {
            tmdb_id: Some(1396),
            release_date: Some("2008-01-01".to_string()),
            ..MediaFactPatch::default()
        };

        assert!(apply_fact_patch_to_series(&mut properties, &patch));
        assert_eq!(properties.tmdb, Some(1396));
        assert_eq!(properties.release_date.as_deref(), Some("2008-01-01"));
    }
}
