use crate::value::CommitTimestamp;
use base64::encode;
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::OffsetComponents;
use google_cloud_googleapis::spanner::v1::struct_type::Field;
use google_cloud_googleapis::spanner::v1::{StructType, Type, TypeCode};
use prost_types::value::Kind;
use prost_types::value::Kind::StringValue;
use prost_types::NullValue::NullValue;
use prost_types::{value, ListValue, Struct, Value};
use std::any::Any;
use std::collections::{BTreeMap, HashMap};

/// A Statement is a SQL query with named parameters.
///
/// A parameter placeholder consists of '@' followed by the parameter name.
/// The parameter name is an identifier which must conform to the naming
/// requirements in https://cloud.google.com/spanner/docs/lexical#identifiers.
/// Parameters may appear anywhere that a literal value is expected. The same
/// parameter name may be used more than once.  It is an error to execute a
/// statement with unbound parameters. On the other hand, it is allowable to
/// bind parameter names that are not used.
///
/// See the documentation of the Row type for how Go types are mapped to Cloud
/// Spanner types.
#[derive(Clone)]
pub struct Statement {
    pub sql: String,
    pub params: BTreeMap<String, Value>,
    pub param_types: HashMap<String, Type>,
}

impl Statement {
    /// new returns a Statement with the given SQL and an empty Params map.
    pub fn new<T: Into<String>>(sql: T) -> Self {
        return Statement {
            sql: sql.into(),
            params: Default::default(),
            param_types: Default::default(),
        };
    }

    /// add_params add the bind parameter.
    /// Implement the ToKind trait to use non-predefined types.
    pub fn add_param<T>(&mut self, name: &str, value: T)
    where
        T: ToKind,
    {
        self.param_types.insert(name.to_string(), T::get_type());
        self.params.insert(
            name.to_string(),
            Value {
                kind: Some(value.to_kind()),
            },
        );
    }
}

fn single_type<T>(code: T) -> Type
where
    T: Into<i32>,
{
    Type {
        code: code.into(),
        array_element_type: None,
        struct_type: None,
    }
}

pub trait ToKind {
    fn to_kind(&self) -> value::Kind;
    fn get_type() -> Type;
}

pub type Kinds = Vec<(&'static str, Kind)>;
pub type Types = Vec<(&'static str, Type)>;

pub trait ToStruct: Sized {
    fn to_kinds(&self) -> Kinds;
    fn get_types() -> Types;
}

impl ToKind for String {
    fn to_kind(&self) -> Kind {
        StringValue(self.clone())
    }
    fn get_type() -> Type {
        single_type(TypeCode::String)
    }
}

impl ToKind for &str {
    fn to_kind(&self) -> Kind {
        StringValue(self.to_string())
    }
    fn get_type() -> Type {
        single_type(TypeCode::String)
    }
}

impl ToKind for i64 {
    fn to_kind(&self) -> Kind {
        self.to_string().to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Int64)
    }
}

impl ToKind for f64 {
    fn to_kind(&self) -> Kind {
        value::Kind::NumberValue(*self)
    }
    fn get_type() -> Type {
        single_type(TypeCode::Float64)
    }
}

impl ToKind for bool {
    fn to_kind(&self) -> Kind {
        value::Kind::BoolValue(*self)
    }
    fn get_type() -> Type {
        single_type(TypeCode::Bool)
    }
}

impl ToKind for NaiveDate {
    fn to_kind(&self) -> Kind {
        self.format("%Y-%m-%d").to_string().to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Date)
    }
}

impl ToKind for NaiveDateTime {
    fn to_kind(&self) -> Kind {
        Utc.from_utc_datetime(self)
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            .to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Timestamp)
    }
}

impl ToKind for CommitTimestamp {
    fn to_kind(&self) -> Kind {
        "spanner.commit_timestamp()".to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Timestamp)
    }
}

impl ToKind for &[u8] {
    fn to_kind(&self) -> Kind {
        base64::encode(self).to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Bytes)
    }
}

impl ToKind for rust_decimal::Decimal {
    fn to_kind(&self) -> Kind {
        self.to_string().to_kind()
    }
    fn get_type() -> Type {
        single_type(TypeCode::Numeric)
    }
}

impl<T> ToKind for T
where
    T: ToStruct,
{
    fn to_kind(&self) -> Kind {
        let mut fields = BTreeMap::<String, Value>::default();
        self.to_kinds().into_iter().for_each(|e| {
            fields.insert(e.0.into(), Value { kind: Some(e.1) });
        });
        Kind::StructValue(Struct { fields })
    }
    fn get_type() -> Type {
        Type {
            code: TypeCode::Struct.into(),
            array_element_type: None,
            struct_type: Some(StructType {
                fields: T::get_types()
                    .into_iter()
                    .map(|t| Field {
                        name: t.0.into(),
                        r#type: Some(t.1),
                    })
                    .collect(),
            }),
        }
    }
}

impl<T> ToKind for Option<T>
where
    T: ToKind,
{
    fn to_kind(&self) -> Kind {
        match self {
            Some(vv) => vv.to_kind(),
            None => value::Kind::NullValue(prost_types::NullValue::NullValue.into()),
        }
    }
    fn get_type() -> Type {
        T::get_type()
    }
}

impl<T> ToKind for Vec<T>
where
    T: ToKind,
{
    fn to_kind(&self) -> Kind {
        value::Kind::ListValue(ListValue {
            values: self
                .into_iter()
                .map(|x| Value {
                    kind: Some(x.to_kind()),
                })
                .collect(),
        })
    }
    fn get_type() -> Type {
        Type {
            code: TypeCode::Array.into(),
            array_element_type: Some(Box::new(T::get_type())),
            struct_type: None,
        }
    }
}