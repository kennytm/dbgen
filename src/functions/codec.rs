//! Encoding and decoding functions.

use data_encoding::{BASE64, BASE64URL_NOPAD, Encoding};
use data_encoding_macro::new_encoding;

use super::{Arguments, Function, args_1};
use crate::{
    bytes::ByteString,
    error::Error,
    eval::{C, CompileContext},
    span::{ResultExt, S, Span},
};

const HEX_ENCODING: Encoding = new_encoding! {
    symbols: "0123456789ABCDEF",
    translate_from: "abcdef",
    translate_to: "ABCDEF",
    ignore: " \t\r\n",
};

const BASE64_ENCODING: Encoding = new_encoding! {
    symbols: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    translate_from: "-_",
    translate_to: "+/",
    ignore: " \t\r\n=",
};

//------------------------------------------------------------------------------

/// The `decode.*` SQL functions
#[derive(Debug)]
pub struct Decode {
    encoding: &'static Encoding,
}

/// The `decode.hex` (a.k.a. `x`) SQL function.
pub const DECODE_HEX: Decode = Decode {
    encoding: &HEX_ENCODING,
};
/// The `decode.base64` and `decode.base64url` SQL functions.
pub const DECODE_BASE64: Decode = Decode {
    encoding: &BASE64_ENCODING,
};

impl Function for Decode {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let encoded = args_1::<ByteString>(span, args, None)?;
        let decoded = self.encoding.decode(encoded.as_bytes()).span_err(span)?;
        Ok(C::Constant(decoded.into()))
    }
}

//------------------------------------------------------------------------------

/// The `encode.*` SQL functions
#[derive(Debug)]
pub struct Encode {
    encoding: &'static Encoding,
}

/// The `encode.hex` SQL function.
pub const ENCODE_HEX: Encode = Encode {
    encoding: &HEX_ENCODING,
};
/// The `encode.base64` SQL function.
pub const ENCODE_BASE64: Encode = Encode { encoding: &BASE64 };
/// The `encode.base64url` SQL function.
pub const ENCODE_BASE64URL: Encode = Encode {
    encoding: &BASE64URL_NOPAD,
};

impl Function for Encode {
    fn compile(&self, _: &CompileContext, span: Span, args: Arguments) -> Result<C, S<Error>> {
        let decoded = args_1::<ByteString>(span, args, None)?;
        let encoded = self.encoding.encode(decoded.as_bytes());
        Ok(C::Constant(encoded.into()))
    }
}
