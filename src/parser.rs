use failure::ResultExt;
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::Parser;
use std::fmt;

use crate::error::{Error, ErrorKind};
use crate::quote::Quote;

#[derive(Parser)]
#[grammar = "parser.pest"]
struct TemplateParser;

#[derive(Debug, Clone)]
pub struct Template {
    pub(crate) table_content: String,
    pub(crate) exprs: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    RowNum,
    String(String),
    Integer(u64),
    Float(f64),
    Function { name: Function, args: Vec<Expr> },
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut res = TemplateParser::parse(Rule::file, input).context(ErrorKind::ParseTemplate)?;
        let mut pairs = res.next().expect("parse rule <file>").into_inner();

        let table_content = pairs
            .next()
            .expect("parse rule <create_table_content>")
            .as_str()
            .to_owned();
        let exprs = Expr::from_pairs(pairs)?;

        Ok(Self { table_content, exprs })
    }
}

impl Expr {
    fn from_pairs(pairs: Pairs<'_, Rule>) -> Result<Vec<Self>, Error> {
        pairs.map(Expr::from_pair).collect()
    }

    fn from_pair(pair: Pair<'_, Rule>) -> Result<Self, Error> {
        match pair.as_rule() {
            Rule::expr_rownum => Ok(Expr::RowNum),
            Rule::expr_function => {
                let mut pairs = pair.into_inner();
                let name = pairs.next().expect("parse rule <function_name>").as_str();
                let name = Function::from_name(name.to_ascii_lowercase())?;
                let args = Expr::from_pairs(pairs)?;
                Ok(Expr::Function { name, args })
            }
            Rule::expr_string => Ok(Expr::String(Quote::Single.unescape(pair.as_str()))),
            Rule::expr_integer => {
                let input = pair.as_str();
                let (base_input, radix) = match input.get(..2) {
                    Some("0x") | Some("0X") => (&input[2..], 16),
                    _ => (input, 10),
                };
                let number = u64::from_str_radix(base_input, radix)
                    .with_context(|_| ErrorKind::IntegerOverflow(input.to_owned()))?;
                Ok(Expr::Integer(number))
            }
            Rule::expr_float => {
                let number = pair.as_str().parse().expect("parse rule <expr_float>");
                Ok(Expr::Float(number))
            }
            Rule::expr_neg => {
                let mut pairs = pair.into_inner();
                let inner = pairs.next().expect("parse rule <expr>");
                let inner = Expr::from_pair(inner)?;
                Ok(Expr::Function {
                    name: Function::Neg,
                    args: vec![inner],
                })
            }
            r => panic!("unexpected rule <{:?}> while parsing an expression", r),
        }
    }
}

macro_rules! define_function {
    (
        pub enum $F:ident {
            $($ident:ident = $s:tt,)+
        }
    ) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum $F {
            $($ident,)+
        }

        impl fmt::Display for $F {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(match self {
                    $($F::$ident => $s,)+
                })
            }
        }

        impl $F {
            fn from_name(name: String) -> Result<Self, Error> {
                Ok(match &*name {
                    $($s => $F::$ident,)+
                    _ => return Err(ErrorKind::UnknownFunction(name).into()),
                })
            }
        }
    }
}

define_function! {
    pub enum Function {
        RandInt = "rand.int",
        RandUInt = "rand.uint",
        RandRegex = "rand.regex",
        RandRange = "rand.range",
        RandRangeInclusive = "rand.range_inclusive",
        RandUniform = "rand.uniform",
        RandUniformInclusive = "rand.uniform_inclusive",

        Neg = "-",
    }
}
