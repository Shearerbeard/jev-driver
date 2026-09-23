//! Serde-name resolution for compile-time backtick validation: the keys
//! the model sees are serde's serialized names, not Rust idents. Mirrors
//! the subset of serde's renaming rules that is statically knowable.

use syn::{Attribute, Field};

/// A field's serialized-key outcome.
pub(crate) enum KeyOutcome {
    /// Serde will emit this key.
    Key(String),
    /// The field is never serialized (`skip` / `skip_serializing`).
    NeverSerialized,
    /// The shape depends on runtime or custom serialization
    /// (`flatten`, `skip_serializing_if`, `serialize_with`) and has no
    /// static answer.
    Opaque,
}

/// One field's ident paired with its resolved outcome.
pub(crate) struct FieldKey {
    /// The Rust field name.
    pub ident: String,
    /// What serde does with it.
    pub outcome: KeyOutcome,
}

impl FieldKey {
    /// True when `token` names this field under either spelling.
    pub fn matches(&self, token: &str) -> bool {
        if token == self.ident {
            return true;
        }
        matches!(&self.outcome, KeyOutcome::Key(k) if k == token)
    }
}

/// Resolves every field's serialized key from the struct's serde
/// attributes. Unparseable or unfamiliar serde attributes are treated as
/// `Opaque`; serde's own derive remains the authority on validity.
pub(crate) fn resolve_fields<'a>(
    container_attrs: &[Attribute],
    fields: impl IntoIterator<Item = &'a Field>,
) -> Vec<FieldKey> {
    let rename_all = container_rename_all(container_attrs);
    fields
        .into_iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?.to_string();
            let outcome = match field_rename(&field.attrs) {
                FieldRename::Renamed(key) => KeyOutcome::Key(key),
                FieldRename::Default => match rename_all.as_ref().map(|rule| rule.apply(&ident)) {
                    Some(key) => KeyOutcome::Key(key),
                    None => KeyOutcome::Key(ident.clone()),
                },
                FieldRename::NeverSerialized => KeyOutcome::NeverSerialized,
                FieldRename::Opaque => KeyOutcome::Opaque,
            };
            Some(FieldKey { ident, outcome })
        })
        .collect()
}

/// The serialized keys a model can reference, or `None` when any field's
/// shape is not statically knowable (exempt from checking).
pub(crate) fn serialized_keys(keys: &[FieldKey]) -> Option<Vec<String>> {
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        match &key.outcome {
            KeyOutcome::Key(k) => out.push(k.clone()),
            KeyOutcome::NeverSerialized => {}
            KeyOutcome::Opaque => return None,
        }
    }
    Some(out)
}

enum FieldRename {
    Default,
    Renamed(String),
    NeverSerialized,
    /// Only `flatten` (or unparseable attrs): the key set itself is
    /// unknowable. Value-shaping attrs (`serialize_with`,
    /// `skip_serializing_if`) leave the key static and do not exempt.
    Opaque,
}
fn field_rename(attrs: &[Attribute]) -> FieldRename {
    let mut outcome = FieldRename::Default;
    let mut skipped = false;
    let mut opaque = false;
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let Ok(()) = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                match field_rename_value(&meta) {
                    Some(key) => outcome = FieldRename::Renamed(key),
                    None => opaque = true,
                }
            } else if meta.path.is_ident("skip") || meta.path.is_ident("skip_serializing") {
                skipped = true;
            } else if meta.path.is_ident("flatten") {
                opaque = true;
            }
            drain_value(&meta);
            Ok(())
        }) else {
            return FieldRename::Opaque;
        };
    }
    // Serde semantics: skip unconditionally wins over rename, whatever
    // the attribute order.
    if skipped {
        FieldRename::NeverSerialized
    } else if opaque {
        FieldRename::Opaque
    } else {
        outcome
    }
}

/// Consumes a `= value` the caller does not interpret, so the meta item
/// counts as fully parsed (syn rejects items with leftover tokens).
fn drain_value(meta: &syn::meta::ParseNestedMeta<'_>) {
    if let Ok(value) = meta.value() {
        drop(value.parse::<syn::LitStr>());
    }
}

/// `rename = "key"` or `rename(serialize = "key")`; `None` for anything
/// else (unparseable, or deserialize-only, which leaves no static
/// serialize-name answer worth guessing at).
fn field_rename_value(meta: &syn::meta::ParseNestedMeta<'_>) -> Option<String> {
    if let Ok(value) = meta.value() {
        let Ok(lit) = value.parse::<syn::LitStr>() else {
            return None;
        };
        return Some(lit.value());
    }
    let mut serialize: Option<String> = None;
    let Ok(()) = meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("serialize") {
            if let Ok(value) = nested.value() {
                if let Ok(lit) = value.parse::<syn::LitStr>() {
                    serialize = Some(lit.value());
                }
            }
        }
        drain_value(&nested);
        Ok(())
    }) else {
        return None;
    };
    serialize
}

fn container_rename_all(attrs: &[Attribute]) -> Option<RenameRule> {
    let mut rule = None;
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        // Literal form resolves directly; the split form
        // `rename_all(serialize = "...", deserialize = "...")` resolves
        // from the serialize side (what the model sees). A rule naming
        // only the deserialize side leaves serialize names at the
        // idents. Malformed input leaves `rule` at None: serde's own
        // derive is the authority and will reject it, so any key we do
        // resolve is moot by then.
        if attr
            .parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    if let Ok(value) = meta.value() {
                        if let Ok(lit) = value.parse::<syn::LitStr>() {
                            rule = RenameRule::parse(&lit.value());
                        }
                    } else if let Ok(()) = meta.parse_nested_meta(|nested| {
                        if nested.path.is_ident("serialize") {
                            if let Ok(value) = nested.value() {
                                if let Ok(lit) = value.parse::<syn::LitStr>() {
                                    rule = RenameRule::parse(&lit.value());
                                }
                            }
                        }
                        drain_value(&nested);
                        Ok(())
                    }) {
                        // Only the deserialize side present: serialize
                        // names stay at the idents (`rule` already None).
                    }
                }
                drain_value(&meta);
                Ok(())
            })
            .is_err()
        {
            return None;
        }
    }
    rule
}

/// The rename_all casing rules serde accepts. Variant names mirror
/// serde's own so the mapping stays obvious.
#[allow(
    clippy::enum_variant_names,
    reason = "the shared `Case` postfix is serde's vocabulary; shortening it would obscure the mapping"
)]
enum RenameRule {
    LowerCase,
    UpperCase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl RenameRule {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "lowercase" => Self::LowerCase,
            "UPPERCASE" => Self::UpperCase,
            "PascalCase" => Self::PascalCase,
            "camelCase" => Self::CamelCase,
            "snake_case" => Self::SnakeCase,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnakeCase,
            "kebab-case" => Self::KebabCase,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebabCase,
            _ => return None,
        })
    }

    /// Serde's field rules assume Rust's snake_case naming convention:
    /// only Pascal/Camel split into words (on `_`/`-` alone — never on
    /// case boundaries); Lower/Snake are identity; the rest are
    /// whole-ident transforms. `XMLHttp` under camelCase is `xMLHttp`;
    /// under snake_case or lowercase it stays `XMLHttp`.
    fn apply(&self, ident: &str) -> String {
        match self {
            Self::LowerCase | Self::SnakeCase => ident.to_owned(),
            Self::UpperCase | Self::ScreamingSnakeCase => ident.to_ascii_uppercase(),
            Self::KebabCase => ident.replace('_', "-"),
            Self::ScreamingKebabCase => ident.replace('_', "-").to_ascii_uppercase(),
            Self::PascalCase => split_words(ident).iter().map(|w| capitalize(w)).collect(),
            Self::CamelCase => split_words(ident)
                .iter()
                .enumerate()
                .map(|(i, word)| {
                    if i == 0 {
                        lowercase_first(word)
                    } else {
                        capitalize(word)
                    }
                })
                .collect(),
        }
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut out: String = c.to_uppercase().collect();
            out.push_str(chars.as_str());
            out
        }
    }
}

/// First char lowercased, rest untouched — serde's first-word camel
/// transform (`XMLHttp` -> `xMLHttp`, not `xmlhttp`).
fn lowercase_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut out: String = c.to_lowercase().collect();
            out.push_str(chars.as_str());
            out
        }
    }
}

/// Splits on `_`/`-` separators only, for the Pascal/Camel rules. Case
/// boundaries do NOT start new words: serde deliberately differs from
/// heck here (`XMLHttp` stays one word, so camelCase yields `xMLHttp`).
fn split_words(ident: &str) -> Vec<String> {
    ident.split(['_', '-']).map(str::to_owned).collect()
}
