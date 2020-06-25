macro_rules! string_enum {
    ($vis:vis enum $name:ident { $($item:ident => $str:expr,)* }) => {
        #[cfg_attr(test, derive(PartialOrd, Ord))]
        #[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
        $vis enum $name {
            $($item,)*
        }

        impl ::std::str::FromStr for $name {
            type Err = ::failure::Error;

            fn from_str(s: &str) -> ::failure::Fallible<$name> {
                match s {
                    $($str => Ok($name::$item),)*
                    s => bail!("invalid {}: {}", stringify!($name), s),
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}", self.to_str())
            }
        }

        impl $name {
            #[allow(dead_code)]
            $vis fn to_str(&self) -> &'static str {
                match *self {
                    $($name::$item => $str,)*
                }
            }

            #[allow(dead_code)]
            $vis fn possible_values() -> &'static [&'static str] {
                &[$($str,)*]
            }
        }

        impl_serde_from_parse!($name, expecting="foo");
    }
}

macro_rules! impl_serde_from_parse {
    ($for:ident, expecting=$expecting:expr) => {
        paste::item! {
            struct [<$for Visitor>];

            impl<'de> ::serde::de::Visitor<'de> for [<$for Visitor>] {
                type Value = $for;

                fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                    f.write_str($expecting)
                }

                fn visit_str<E: ::serde::de::Error>(self, input: &str) -> Result<$for, E> {
                    use std::str::FromStr;
                    $for::from_str(input).map_err(E::custom)
                }
            }
        }

        impl<'de> ::serde::de::Deserialize<'de> for $for {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::de::Deserializer<'de>,
            {
                deserializer.deserialize_str(paste::expr! { [<$for Visitor>] })
            }
        }

        impl ::serde::ser::Serialize for $for {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::ser::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }
    };
}

macro_rules! btreeset {
    ($($x:expr),+ $(,)?) => (
        vec![$($x),+].into_iter().collect::<BTreeSet<_>>()
    );
}

macro_rules! impl_ord_from_serialize {
    ($name:ident) => {
        impl ::std::cmp::PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                serde_json::to_string(self)
                    .unwrap()
                    .eq(&serde_json::to_string(other).unwrap())
            }
        }

        impl ::std::cmp::Eq for $name {}

        impl ::std::cmp::PartialOrd for $name {
            fn partial_cmp(&self, other: &$name) -> Option<::std::cmp::Ordering> {
                serde_json::to_string(self)
                    .unwrap()
                    .partial_cmp(&serde_json::to_string(other).unwrap())
            }
        }

        impl ::std::cmp::Ord for $name {
            fn cmp(&self, other: &$name) -> ::std::cmp::Ordering {
                serde_json::to_string(self)
                    .unwrap()
                    .cmp(&serde_json::to_string(other).unwrap())
            }
        }
    };
}
