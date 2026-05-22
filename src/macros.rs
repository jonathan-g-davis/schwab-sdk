//! Crate-private declarative macros shared across the API endpoint modules.

/// Declare a public string-valued enum with a `Unknown(String)` catch-all so
/// wire values added after this crate was published deserialize cleanly,
/// plus the matching `Serialize` / `Deserialize` / `From<X> for String`
/// / `From<String> for X` impls.
///
/// The expansion requires `serde::{Serialize, Deserialize}` and the `strum`
/// derive macros to be reachable at the call site (Rust resolves derive
/// macros at the use site, not the definition site, so the macro deliberately
/// emits the unqualified `Serialize` / `Deserialize` paths the call site is
/// expected to have in scope).
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident = $wire:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash,
            strum::Display, strum::EnumString,
            Serialize, Deserialize,
        )]
        #[serde(into = "String", from = "String")]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                #[strum(serialize = $wire)]
                $variant,
            )*
            #[strum(default)]
            Unknown(String),
        }

        impl From<$name> for String {
            fn from(v: $name) -> Self { v.to_string() }
        }

        impl From<String> for $name {
            fn from(v: String) -> Self {
                v.parse().expect(concat!(stringify!($name), " FromStr is infallible (strum default)"))
            }
        }
    };
}

pub(crate) use string_enum;
