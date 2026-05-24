//! morsel-bake — offline CLI that turns trained safetensors weights
//! into a standalone Rust source file containing `pub const` arrays.
//!
//! The library half (this file) is the pure-logic core: it reads a
//! safetensors buffer, validates it, sanitizes tensor names to
//! Rust-safe `UPPER_SNAKE_CASE` identifiers, and renders Rust source
//! that preserves f32 bit-exactness via 9-significant-digit decimals
//! (the round-trip-safe shortest formatter for `f32`).
//!
//! Errors map to fixed exit codes via [`BakeError::exit_code`] so the
//! binary layer can stay tiny and the tests can assert on numeric
//! status without scraping prose.

#![cfg_attr(not(test), forbid(unsafe_code))]

use std::collections::BTreeMap;

use chrono::SecondsFormat;
use safetensors::{Dtype, SafeTensors};
use thiserror::Error;

/// Maximum tensor rank that Phase 0a will emit. Anything higher
/// becomes a [`BakeError::RankTooHigh`] so the runtime never has to
/// reason about arbitrary-rank const arrays.
pub const MAX_RANK: usize = 4;

/// All failure modes the bake pipeline can surface. The numeric
/// `exit_code` is part of the public contract (acceptance tests pin it).
#[derive(Debug, Error)]
pub enum BakeError {
    /// I/O failure reading the input or writing the output.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// safetensors header / payload parse failure.
    #[error("safetensors parse: {0}")]
    Parse(#[from] safetensors::SafeTensorError),

    /// `--in` path could not be opened. Distinct from generic
    /// [`BakeError::Io`] because AC7 requires exit code 2 and a
    /// specific stderr string.
    #[error("safetensors file not found: {0}")]
    InputMissing(String),

    /// Two distinct tensor names collapse to the same Rust ident
    /// after sanitization. AC4 requires exit 4.
    #[error("name collision after sanitization: {sanitized} <- {originals:?}")]
    NameCollision {
        /// The sanitized identifier that two originals both produced.
        sanitized: String,
        /// The original tensor names that collided.
        originals: Vec<String>,
    },

    /// Tensor dtype is something other than `F32`. AC5 requires exit 5.
    #[error("only f32 supported in Phase 0a: tensor {name} has dtype {dtype}")]
    UnsupportedDtype {
        /// Tensor whose dtype was rejected.
        name: String,
        /// Human-readable form of the rejected dtype.
        dtype: String,
    },

    /// Tensor rank exceeds [`MAX_RANK`]. AC6 requires exit 6 and the
    /// exact stderr phrasing `rank N exceeds supported max 4`.
    #[error("rank {rank} exceeds supported max {max}")]
    RankTooHigh {
        /// Offending tensor's rank.
        rank: usize,
        /// Maximum supported rank ([`MAX_RANK`]).
        max: usize,
    },

    /// `--out` path exists and `--force` was not supplied. AC7 exit 7.
    #[error("output file already exists (pass --force to overwrite): {0}")]
    OutputExists(String),
}

impl BakeError {
    /// Maps each error variant to its acceptance-contract exit code.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::InputMissing(_) => 2,
            Self::NameCollision { .. } => 4,
            Self::UnsupportedDtype { .. } => 5,
            Self::RankTooHigh { .. } => 6,
            Self::OutputExists(_) => 7,
            Self::Io(_) | Self::Parse(_) => 1,
        }
    }
}

/// Crate version reported in the generated source's doc header.
#[must_use]
pub const fn crate_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Sanitize a safetensors tensor name to a Rust-identifier-safe
/// `UPPER_SNAKE_CASE` form.
///
/// - `.` and `/` become `_`
/// - any other non-alphanumeric / non-underscore char becomes `_`
/// - result is uppercased
/// - if the first char would be a digit, an `_` is prepended so it's
///   a valid Rust identifier
/// - empty input yields a single `_` (still a legal identifier, and
///   collision-detection will catch overlap with any real name)
#[must_use]
pub fn sanitize_ident(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return "_".to_string();
    }
    let first_is_digit = out.chars().next().is_some_and(|c| c.is_ascii_digit());
    if first_is_digit {
        let mut prefixed = String::with_capacity(out.len() + 1);
        prefixed.push('_');
        prefixed.push_str(&out);
        return prefixed;
    }
    out
}

/// Result of the bake — the rendered Rust source plus a copy of the
/// resolved sanitized names (kept for downstream tooling / diagnostics).
#[derive(Debug)]
pub struct BakeOutput {
    /// The generated `.rs` source.
    pub source: String,
    /// Sanitized const names, in emission order.
    pub idents: Vec<String>,
}

/// Inputs that affect how the source is rendered but aren't part of
/// the tensor payload — kept in a struct so the function signature
/// doesn't drift past the clippy `too_many_arguments` threshold.
#[derive(Debug, Clone)]
pub struct BakeOptions<'a> {
    /// User-facing arch name; baked into `ARCH_FINGERPRINT`.
    pub arch: &'a str,
    /// Basename of the input file (no directory); shown in the doc header.
    pub input_basename: &'a str,
    /// Override for the `@ <timestamp>` in the doc header. `None` means
    /// "now". The test suite passes a fixed value to make AC8's stable
    /// rebake byte-comparable.
    pub source_stamp: Option<&'a str>,
}

/// Bake a safetensors buffer into Rust source.
///
/// # Errors
///
/// Returns [`BakeError`] for parse failures, unsupported dtypes,
/// rank > [`MAX_RANK`], or name collisions after sanitization.
pub fn bake(buffer: &[u8], opts: &BakeOptions<'_>) -> Result<BakeOutput, BakeError> {
    let st = SafeTensors::deserialize(buffer)?;

    // Sort by original name for deterministic emission order. The
    // safetensors crate sorts internally by dtype for storage layout,
    // but we want a stable *source* layout that doesn't depend on
    // dtype tiebreaks.
    let mut tensors: Vec<_> = st.tensors();
    tensors.sort_by(|a, b| a.0.cmp(&b.0));

    // First pass: dtype + rank validation.
    for (name, view) in &tensors {
        if view.dtype() != Dtype::F32 {
            return Err(BakeError::UnsupportedDtype {
                name: name.clone(),
                dtype: format!("{:?}", view.dtype()),
            });
        }
        let rank = view.shape().len();
        if rank > MAX_RANK {
            return Err(BakeError::RankTooHigh {
                rank,
                max: MAX_RANK,
            });
        }
    }

    // Second pass: sanitize + collision detection.
    let mut by_sanitized: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, _) in &tensors {
        let sanitized = sanitize_ident(name);
        by_sanitized
            .entry(sanitized)
            .or_default()
            .push(name.clone());
    }
    for (sanitized, originals) in &by_sanitized {
        if originals.len() > 1 {
            let mut sorted = originals.clone();
            sorted.sort();
            return Err(BakeError::NameCollision {
                sanitized: sanitized.clone(),
                originals: sorted,
            });
        }
    }

    // Render. `stamp_owned` only materializes when no override is
    // supplied; the `match` form sidesteps the `option_if_let_else`
    // lint while still avoiding an unnecessary allocation in the
    // overridden path.
    let stamp_owned: String = opts.source_stamp.map_or_else(
        || chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        std::string::ToString::to_string,
    );
    let stamp: &str = &stamp_owned;

    let mut src = String::new();
    src.push_str(&format!(
        "//! generated by morsel-bake v{} from {} @ {}\n\n",
        crate_version(),
        opts.input_basename,
        stamp,
    ));

    let mut dim_summary_parts: Vec<String> = Vec::with_capacity(tensors.len());
    let mut idents: Vec<String> = Vec::with_capacity(tensors.len());

    for (name, view) in &tensors {
        let ident = sanitize_ident(name);
        let shape = view.shape();
        let values = decode_f32_le(view.data(), shape)?;
        render_const(&mut src, &ident, shape, &values);
        idents.push(ident);
        dim_summary_parts.push(shape_summary(shape));
    }

    let dim_summary = dim_summary_parts.join("-");
    let fingerprint = if dim_summary.is_empty() {
        format!("{}-v1", opts.arch)
    } else {
        format!("{}-{dim_summary}-v1", opts.arch)
    };
    src.push_str(&format!(
        "pub const ARCH_FINGERPRINT: &str = {fingerprint:?};\n"
    ));

    Ok(BakeOutput {
        source: src,
        idents,
    })
}

/// Decode a contiguous little-endian f32 byte slice into a `Vec<f32>`.
/// Returns a parse error if the byte length doesn't match the shape.
fn decode_f32_le(bytes: &[u8], shape: &[usize]) -> Result<Vec<f32>, BakeError> {
    let elem_count: usize = shape.iter().product();
    let expected_bytes = elem_count.checked_mul(4).unwrap_or(0);
    if bytes.len() != expected_bytes {
        return Err(BakeError::Parse(
            safetensors::SafeTensorError::InvalidHeaderLength,
        ));
    }
    let mut out = Vec::with_capacity(elem_count);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().map_err(|_| {
            BakeError::Parse(safetensors::SafeTensorError::InvalidHeaderLength)
        })?;
        out.push(f32::from_le_bytes(arr));
    }
    Ok(out)
}

/// Render `pub const NAME: <type> = <literal>;` into `out`.
fn render_const(out: &mut String, ident: &str, shape: &[usize], values: &[f32]) {
    let ty = type_for(shape);
    out.push_str(&format!("pub const {ident}: {ty} = "));
    render_literal(out, shape, values, 0);
    out.push_str(";\n");
}

/// Build the Rust type for a tensor shape: `f32`, `[f32; N]`,
/// `[[f32; D1]; D0]`, etc.
fn type_for(shape: &[usize]) -> String {
    if shape.is_empty() {
        return "f32".to_string();
    }
    let mut ty = String::from("f32");
    for &d in shape.iter().rev() {
        ty = format!("[{ty}; {d}]");
    }
    ty
}

/// Render a tensor's nested-array literal. Recursive over rank.
fn render_literal(out: &mut String, shape: &[usize], values: &[f32], offset_in_elems: usize) {
    if shape.is_empty() {
        // Rank-0: emit the single scalar.
        let v = values.first().copied().unwrap_or(0.0_f32);
        out.push_str(&format_f32(v));
        return;
    }

    if shape.len() == 1 {
        let n = shape.first().copied().unwrap_or(0);
        out.push('[');
        for i in 0..n {
            if i > 0 {
                out.push_str(", ");
            }
            let v = values.get(offset_in_elems + i).copied().unwrap_or(0.0_f32);
            out.push_str(&format_f32(v));
        }
        out.push(']');
        return;
    }

    // Multi-dim: outer dim is shape[0], inner block size is product of the rest.
    let (outer, rest) = shape.split_first().unwrap_or((&0, &[]));
    let inner_count: usize = rest.iter().product();
    out.push('[');
    for i in 0..*outer {
        if i > 0 {
            out.push_str(", ");
        }
        render_literal(out, rest, values, offset_in_elems + i * inner_count);
    }
    out.push(']');
}

/// Format an f32 so the parser recovers the exact original bits.
/// Special-cases inf/nan because the default `{:e}` prints `inf` /
/// `NaN` which Rust won't parse as an f32 literal. We use 9
/// significant digits which is the round-trip-safe minimum for f32.
fn format_f32(v: f32) -> String {
    if v.is_nan() {
        return "f32::NAN".to_string();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() {
            "f32::NEG_INFINITY".to_string()
        } else {
            "f32::INFINITY".to_string()
        };
    }
    // {:e} uses minimal digits; we force 8 fractional => 9 sigdigits,
    // which is the round-trip-safe minimum for f32.
    let s = format!("{v:.8e}_f32");
    s
}

/// Short shape tag for the fingerprint: scalar -> "scalar",
/// 1-D length 5 -> "5", 2-D 256x64 -> "256x64".
fn shape_summary(shape: &[usize]) -> String {
    if shape.is_empty() {
        return "scalar".to_string();
    }
    shape
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("x")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_basic() {
        assert_eq!(sanitize_ident("weight.ih"), "WEIGHT_IH");
        assert_eq!(sanitize_ident("layer/0/weight"), "LAYER_0_WEIGHT");
        assert_eq!(sanitize_ident("bn1.running_mean"), "BN1_RUNNING_MEAN");
        assert_eq!(sanitize_ident("0_first"), "_0_FIRST");
        assert_eq!(sanitize_ident(""), "_");
    }

    #[test]
    fn type_for_shapes() {
        assert_eq!(type_for(&[]), "f32");
        assert_eq!(type_for(&[5]), "[f32; 5]");
        assert_eq!(type_for(&[2, 3]), "[[f32; 3]; 2]");
        assert_eq!(type_for(&[2, 3, 4]), "[[[f32; 4]; 3]; 2]");
        assert_eq!(type_for(&[1, 2, 3, 4]), "[[[[f32; 4]; 3]; 2]; 1]");
    }

    #[test]
    fn f32_round_trip_decimal() {
        // The whole f32 universe via .parse::<f32>() of our formatter.
        for bits in [
            0u32,
            1,
            0x7f7f_ffff,
            0x3f80_0000,
            0xbf80_0000,
            0x0080_0000,
            0x4049_0fdb, // ~pi
        ] {
            let v = f32::from_bits(bits);
            let s = format_f32(v);
            let parsed: f32 = s.trim_end_matches("_f32").parse().expect("parse f32");
            assert_eq!(parsed.to_bits(), v.to_bits(), "bits {bits:#010x} -> {s}");
        }
    }

    #[test]
    fn shape_summary_works() {
        assert_eq!(shape_summary(&[]), "scalar");
        assert_eq!(shape_summary(&[5]), "5");
        assert_eq!(shape_summary(&[256, 64]), "256x64");
    }
}
