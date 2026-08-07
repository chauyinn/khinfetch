use crate::{
    domain::{AppError, AppResult, ErrorCode, QualityPreference},
    khinsider::ResolvedVariant,
};

pub(crate) fn select_variant(
    variants: &[ResolvedVariant],
    quality: QualityPreference,
) -> AppResult<ResolvedVariant> {
    let selected = match quality {
        QualityPreference::Mp3 => variants
            .iter()
            .find(|variant| variant.format.eq_ignore_ascii_case("MP3")),
        QualityPreference::BestAvailable => variants
            .iter()
            .filter_map(|variant| format_rank(&variant.format).map(|rank| (rank, variant)))
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, variant)| variant),
    };
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

fn format_rank(format: &str) -> Option<u8> {
    match format.to_ascii_uppercase().as_str() {
        "FLAC" => Some(0),
        "ALAC" => Some(1),
        "AAC" | "M4A" => Some(2),
        "MP3" => Some(3),
        _ => None,
    }
}
