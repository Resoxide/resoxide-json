use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use rust_decimal::Decimal;
use serde::Deserialize;
pub use resoxide_json_procmacro::Json;

#[derive(Debug,Default,Copy,Clone)]
pub struct Error;

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"Json-Error")
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Error
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(_: std::num::TryFromIntError) -> Self {
        Error
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(_: std::num::ParseIntError) -> Self {
        Error
    }
}

impl From<std::num::ParseFloatError> for Error {
    fn from(_: std::num::ParseFloatError) -> Self {
        Error
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub type Result<T,E = Error> = std::result::Result<T,E>;

pub trait Json: Sized {
    type Error;

    fn to_token(&self) -> Result<Token, Self::Error>;
    fn from_token(token: &Token) -> Result<Self, Self::Error>;
    fn error() -> Self::Error;
}

#[derive(Debug,Clone)]
pub struct Number(serde_json::Number);

impl Number {
    fn as_i64(&self) -> Result<i64, Error> {
        self.0.as_i64().ok_or(Error)
    }

    fn as_u64(&self) -> Result<u64, Error> {
        self.0.as_u64().ok_or(Error)
    }

    fn as_f64(&self) -> Result<f64, Error> {
        self.0.as_f64().ok_or(Error)
    }

    fn from_i64(i: i64) -> Self {
        Self(serde_json::Number::from(i))
    }

    fn from_u64(u: u64) -> Self {
        Self(serde_json::Number::from(u))
    }

    fn from_f64(f: f64) -> Result<Self> {
        Ok(Self(serde_json::Number::from_f64(f).ok_or(Error)?))
    }
}

#[derive(Debug,Clone)]
pub enum Token {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Token>),
    Object(HashMap<String, Token>),
}

impl From<&Token> for serde_json::Value {
    fn from(token: &Token) -> Self {
        match token {
            Token::Null => serde_json::Value::Null,
            Token::Bool(b) => serde_json::Value::Bool(*b),
            Token::Number(n) => serde_json::Value::Number(n.0.clone()),
            Token::String(s) => serde_json::Value::String(s.clone()),
            Token::Array(tokens) => serde_json::Value::Array(tokens.iter().map(<&Token>::into).collect()),
            Token::Object(map) => serde_json::Value::Object(map.iter()
                .map(|(k,v)|(k.clone(), v.into())).collect()),
        }
    }
}

impl From<&serde_json::Value> for Token {
    fn from(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Token::Null,
            serde_json::Value::Bool(b) => Token::Bool(*b),
            serde_json::Value::Number(n) => Token::Number(Number(n.clone())),
            serde_json::Value::String(s) => Token::String(s.clone()),
            serde_json::Value::Array(a) => Token::Array(a.iter().map(Token::from).collect()),
            serde_json::Value::Object(map) => Token::Object(map.iter()
                .map(|(k,v)|(k.clone(), v.into())).collect()),
        }
    }
}

impl Token {
    pub fn serialize(&self) -> Result<String> {
        Ok(serde_json::to_string(&serde_json::Value::from(self))?)
    }

    pub fn deserialize_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Token::from(&serde_json::from_slice::<serde_json::Value>(bytes)?))
    }

    pub fn deserialize_str(str: &str) -> Result<Self> {
        Ok(Token::from(&serde_json::from_str::<serde_json::Value>(str)?))
    }
}

impl Json for Token {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(self.clone())
    }

    fn from_token(token: &Token) -> Result<Token, Self::Error> {
        Ok(token.clone())
    }

    fn error() -> Self::Error {
        Error
    }
}

impl Json for serde_json::Value {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(self.into())
    }

    fn from_token(token: &Token) -> Result<Self, Self::Error> {
        Ok(token.into())
    }

    fn error() -> Self::Error {
        Error
    }
}

macro_rules! impl_json {
    ($t:ident,$t64:ident,$from:ident,$as:ident) => {
        impl Json for $t {
            type Error = Error;

            fn to_token(&self) -> Result<Token, Self::Error> {
                Ok(Token::Number(Number::$from(*self as $t64)))
            }

            fn from_token(token: &Token) -> Result<Self> {
                match token {
                    Token::Number(n) => Ok($t::try_from(n.$as()?)?),
                    Token::String(s) => Ok(s.parse()?),
                    _ => Err(Error),
                }
            }

            fn error() -> Self::Error {
                Error
            }
        }
    }
}

impl_json!(u8, u64, from_u64, as_u64);
impl_json!(u16, u64, from_u64, as_u64);
impl_json!(u32, u64, from_u64, as_u64);
impl_json!(u64, u64, from_u64, as_u64);
impl_json!(i8, i64, from_i64, as_i64);
impl_json!(i16, i64, from_i64, as_i64);
impl_json!(i32, i64, from_i64, as_i64);
impl_json!(i64, i64, from_i64, as_i64);

impl Json for bool {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(Token::Bool(*self))
    }

    fn from_token(token: &Token) -> Result<Self> {
        match token {
            Token::Bool(b) => Ok(*b),
            _ => Err(Error),
        }
    }

    fn error() -> Self::Error {
        Error
    }
}

impl Json for f64 {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        if self.is_finite() {
            Ok(Token::Number(Number::from_f64(*self)?))
        } else if self.is_infinite() {
            if self.is_sign_positive() {
                Ok(Token::String("Infinity".to_string()))
            } else {
                Ok(Token::String("-Infinity".to_string()))
            }
        } else if self.is_nan() {
            Ok(Token::String("NaN".to_string()))
        } else {
            Err(Error)
        }
    }

    fn from_token(token: &Token) -> Result<Self> {
        match token {
            Token::Number(n) => Ok(n.as_f64()?),
            Token::String(s) => match s.as_str() {
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                "NaN" => Ok(f64::NAN),
                s => Ok(s.parse()?),
            },
            _ => Err(Error),
        }
    }

    fn error() -> Self::Error {
        Error
    }
}

impl Json for f32 {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        if self.is_finite() {
            Ok(Token::Number(Number::from_f64(*self as f64)?))
        } else if self.is_infinite() {
            if self.is_sign_positive() {
                Ok(Token::String("Infinity".to_string()))
            } else {
                Ok(Token::String("-Infinity".to_string()))
            }
        } else if self.is_nan() {
            Ok(Token::String("NaN".to_string()))
        } else {
            Err(Error)
        }
    }

    fn from_token(token: &Token) -> Result<Self> {
        match token {
            Token::Number(n) => Ok(n.as_f64()? as f32),
            Token::String(s) => match s.as_str() {
                "Infinity" => Ok(f32::INFINITY),
                "-Infinity" => Ok(f32::NEG_INFINITY),
                "NaN" => Ok(f32::NAN),
                s => Ok(s.parse()?),
            },
            _ => Err(Error),
        }
    }

    fn error() -> Self::Error {
        Error
    }
}

impl Json for String {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(Token::String(self.clone()))
    }

    fn from_token(token: &Token) -> Result<Self, Self::Error> {
        match token {
            Token::String(s) => Ok(s.clone()),
            _ => Err(Error),
        }
    }

    fn error() -> Self::Error {
        Error
    }
}

impl Json for Decimal {
    type Error = Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(Token::from(&serde_json::to_value(self)?))
    }

    fn from_token(token: &Token) -> Result<Self> {
        Ok(<Decimal as Deserialize>::deserialize(&serde_json::Value::from(token))?)
    }

    fn error() -> Self::Error {
        Error
    }
}

impl<T: Json> Json for Option<T> {
    type Error = T::Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        match self {
            None => Ok(Token::Null),
            Some(v) => v.to_token(),
        }
    }

    fn from_token(token: &Token) -> Result<Self, Self::Error> {
        match token {
            Token::Null => Ok(None),
            _ => Ok(Some(T::from_token(token)?)),
        }
    }

    fn error() -> Self::Error {
        T::error()
    }
}

impl<T: Json> Json for Vec<T> {
    type Error = T::Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(Token::Array(self.iter().map(|v| v.to_token()).collect::<Result<Vec<_>, _>>()?))
    }

    fn from_token(token: &Token) -> Result<Self, Self::Error> {
        match token {
            Token::Array(v) => Ok(v.iter().map(|t| T::from_token(t)).collect::<Result<Vec<_>, _>>()?),
            _ => Err(T::error()),
        }
    }

    fn error() -> Self::Error {
        T::error()
    }
}

impl<T: Json> Json for HashMap<String, T> {
    type Error = T::Error;

    fn to_token(&self) -> Result<Token, Self::Error> {
        Ok(Token::Object(self.iter().map(|(k, v)| Ok((k.to_string(), v.to_token()?))).collect::<Result<HashMap<_, _>, _>>()?))
    }

    fn from_token(token: &Token) -> Result<Self, Self::Error> {
        match token {
            Token::Object(m) => Ok(m.iter().map(|(k,t)| Ok((k.to_string(), T::from_token(t)?))).collect::<Result<HashMap<_, _>, _>>()?),
            _ => Err(T::error()),
        }
    }

    fn error() -> Self::Error {
        T::error()
    }
}
