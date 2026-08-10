use crate::{
    domain::{AppError, AppResult, ErrorCode, QualityPreference},
    khinsider::ResolvedVariant,
};

pub(crate) fn select_variant(
    variants: &[ResolvedVariant],
    quality: QualityPreference,
) -> AppResult<ResolvedVariant> {
    let selected = select_format(
        variants.iter().map(|variant| variant.format.as_str()),
        quality,
    )
    .and_then(|format| {
        variants
            .iter()
            .find(|variant| variant.format.eq_ignore_ascii_case(format))
    });
    selected.cloned().ok_or_else(|| {
        AppError::new(
            ErrorCode::UnsupportedFormat,
            match quality {
                QualityPreference::Mp3 => "该曲目没有可用的 MP3 文件",
                QualityPreference::BestAvailable => "该曲目没有受支持的音频格式",
            },
            false,
        )
    })
}

pub(crate) fn format_rank(format: &str) -> Option<u8> {
    match format.to_ascii_uppercase().as_str() {
        "FLAC" => Some(0),
        "ALAC" => Some(1),
        "OGG" => Some(2),
        "AAC" | "M4A" => Some(3),
        "MP3" => Some(4),
        _ => None,
    }
}

pub(crate) fn select_format<'a>(
    formats: impl IntoIterator<Item = &'a str>,
    quality: QualityPreference,
) -> Option<&'a str> {
    match quality {
        QualityPreference::Mp3 => formats
            .into_iter()
            .find(|format| format.eq_ignore_ascii_case("MP3")),
        QualityPreference::BestAvailable => formats
            .into_iter()
            .filter_map(|format| format_rank(format).map(|rank| (rank, format)))
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, format)| format),
    }
}

#[cfg(test)]
mod tests {
    use super::select_format;
    use crate::domain::QualityPreference;

    #[test]
    fn best_available_prefers_ogg_over_mp3() {
        let selected = select_format(["MP3", "OGG"], QualityPreference::BestAvailable);
        assert_eq!(selected, Some("OGG"));
    }

    #[test]
    fn mp3_preference_requires_mp3() {
        let selected = select_format(["FLAC", "OGG"], QualityPreference::Mp3);
        assert_eq!(selected, None);
    }
}
