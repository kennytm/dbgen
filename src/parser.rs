use crate::{
    error::{Error, ErrorKind},
    value::Value,
};
use failure::ResultExt;
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use pest_derive::Parser;
use std::{collections::HashMap, fmt};

#[derive(Parser)]
#[grammar = "parser.pest"]
struct TemplateParser;

/// A schema-qualified name with quotation marks still intact.
#[derive(Debug, Clone)]
pub struct QName {
    pub database: Option<String>,
    pub schema: Option<String>,
    pub table: String,
}

impl QName {
    fn from_pairs(mut pairs: Pairs<'_, Rule>) -> Self {
        let mut qname = Self {
            database: None,
            schema: None,
            table: pairs.next().expect("at least one name").as_str().to_owned(),
        };
        if let Some(pair) = pairs.next() {
            qname.schema = Some(qname.table);
            qname.table = pair.as_str().to_owned();
            if let Some(pair) = pairs.next() {
                qname.database = qname.schema;
                qname.schema = Some(qname.table);
                qname.table = pair.as_str().to_owned();
            }
        }
        qname
    }

    fn estimated_joined_len(&self) -> usize {
        self.database.as_ref().map_or(0, |s| s.len() + 1)
            + self.schema.as_ref().map_or(0, |s| s.len() + 1)
            + self.table.len()
    }

    /// Parses a qualified name
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut pairs = TemplateParser::parse(Rule::qname, input).context(ErrorKind::ParseTemplate)?;
        Ok(Self::from_pairs(pairs.next().unwrap().into_inner()))
    }

    /// Obtains the qualified name connected with dots (`"db"."schema"."table"`)
    pub fn qualified_name(&self) -> String {
        let mut res = String::with_capacity(self.estimated_joined_len());
        if let Some(db) = &self.database {
            res.push_str(db);
            res.push('.');
        }
        if let Some(schema) = &self.schema {
            res.push_str(schema);
            res.push('.');
        }
        res.push_str(&self.table);
        res
    }

    /// Obtains the unique name.
    ///
    /// This name is transformed from the qualified name with these changes:
    ///  - Unquoted names are all converted to lower case in the default
    ///     collation (`XyzÄbc` → `xyzäbc`). If the lowercasing results in
    ///     multiple characters (e.g. `İ` → `i̇`), only the first will be
    ///     included.
    ///  - Quotation marks are removed (`"Hello ""world"""` → `Hello "world"`)
    ///  - Special characters including `.`, `-` and `/` are percent-encoded,
    ///     so the resulting string can be safely used as a filename.
    pub fn unique_name(&self) -> String {
        let mut res = String::with_capacity(self.estimated_joined_len());

        if let Some(db) = &self.database {
            unescape_into(&mut res, db, true);
            res.push('.');
        }
        if let Some(schema) = &self.schema {
            unescape_into(&mut res, schema, true);
            res.push('.');
        }
        unescape_into(&mut res, &self.table, true);
        res
    }
}

fn unescape_into(res: &mut String, ident: &str, do_percent_escape: bool) {
    use std::fmt::Write;

    let mut chars = ident.chars();
    let escape_char = match chars.next() {
        c @ Some('`') | c @ Some('\'') | c @ Some('"') => c,
        Some('[') => Some(']'),
        _ => {
            chars = ident.chars();
            None
        }
    };
    let mut pass_through_escape_char = false;
    for mut c in chars {
        if pass_through_escape_char {
            pass_through_escape_char = false;
        } else if Some(c) == escape_char {
            pass_through_escape_char = true;
            continue;
        } else if escape_char.is_none() {
            c = c.to_lowercase().next().unwrap();
        }
        match c {
            '.' | '-' | '/' => {
                if do_percent_escape {
                    write!(res, "%{:02X}", c as u32).unwrap();
                    continue;
                }
            }
            _ => {}
        }
        res.push(c);
    }
}

#[derive(Debug, Clone)]
pub struct Template {
    /// The default table name.
    pub name: QName,

    /// The content of the CREATE TABLE statement.
    pub content: String,

    /// The expressions to populate the table.
    pub exprs: Vec<Expr>,

    /// Number of variables involved in the expressions.
    pub variables_count: usize,
}

#[derive(Debug, Clone)]
pub enum Expr {
    RowNum,
    Value(Value),
    GetVariable(usize),
    SetVariable(usize, Box<Expr>),
    Function { name: Function, args: Vec<Expr> },
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut pairs = TemplateParser::parse(Rule::create_table, input).context(ErrorKind::ParseTemplate)?;

        let name = QName::from_pairs(pairs.next().unwrap().into_inner());

        let mut alloc = Allocator::default();
        let mut exprs = Vec::new();
        let mut content = String::from("(");

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::column_definition => {
                    content.push_str(pair.as_str());
                }
                Rule::table_options => {
                    content.push(')');
                    content.push_str(pair.as_str());
                }
                _ => {
                    exprs.push(alloc.expr_from_pair(pair)?);
                }
            }
        }

        Ok(Self {
            name,
            content,
            exprs,
            variables_count: alloc.count,
        })
    }
}

#[derive(Default)]
struct Allocator {
    count: usize,
    map: HashMap<String, usize>,
}

impl Allocator {
    fn allocate(&mut self, var_name: String) -> usize {
        let count = &mut self.count;
        *self.map.entry(var_name).or_insert_with(|| {
            let last = *count;
            *count += 1;
            last
        })
    }

    fn expr_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Vec<Expr>, Error> {
        pairs.map(|p| self.expr_from_pair(p)).collect()
    }

    fn expr_from_pair(&mut self, pair: Pair<'_, Rule>) -> Result<Expr, Error> {
        match pair.as_rule() {
            Rule::expr_rownum => Ok(Expr::RowNum),
            Rule::expr_null => Ok(Expr::Value(Value::Null)),
            Rule::expr_true => Ok(Expr::Value(1_u64.into())),
            Rule::expr_false => Ok(Expr::Value(0_u64.into())),
            Rule::expr_function => {
                let mut pairs = pair.into_inner();
                let q_name = QName::from_pairs(pairs.next().unwrap().into_inner());
                let name = Function::from_name(q_name.unique_name())?;
                let args = self.expr_from_pairs(pairs)?;
                Ok(Expr::Function { name, args })
            }
            Rule::expr_string => {
                let mut string = String::with_capacity(pair.as_str().len());
                unescape_into(&mut string, pair.as_str(), false);
                Ok(Expr::Value(string.into()))
            }
            Rule::expr_number => parse_number(pair.as_str()).map(Expr::Value),
            Rule::expr_cmp => {
                let mut pairs = pair.into_inner();
                let left = self.expr_from_pair(pairs.next().unwrap())?;
                let op = pairs.next().unwrap().as_str();
                let right = self.expr_from_pair(pairs.next().unwrap())?;
                Ok(Expr::Function {
                    name: Function::from_op(op).unwrap_or(Function::IsNot),
                    args: vec![left, right],
                })
            }
            Rule::expr_plus | Rule::expr_mul => {
                let mut pairs = pair.into_inner();
                let mut args = Vec::with_capacity(2);
                args.push(self.expr_from_pair(pairs.next().unwrap())?);
                let mut name = Function::from_op(pairs.next().unwrap().as_str()).unwrap();
                args.push(self.expr_from_pair(pairs.next().unwrap())?);
                for pair in pairs {
                    match pair.as_rule() {
                        Rule::plus_op | Rule::mul_op => {
                            let cur_name = Function::from_op(pair.as_str()).unwrap();
                            if cur_name != name {
                                let collapsed = Expr::Function { name, args };
                                name = cur_name;
                                args = Vec::with_capacity(2);
                                args.push(collapsed);
                            }
                        }
                        _ => {
                            args.push(self.expr_from_pair(pair)?);
                        }
                    }
                }
                Ok(Expr::Function { name, args })
            }
            Rule::expr_set_variable | Rule::expr_get_variable => {
                let mut pairs = pair.into_inner();
                let ident_str = pairs.next().unwrap().as_str();
                let mut ident = String::with_capacity(ident_str.len());
                unescape_into(&mut ident, ident_str, false);
                let var_index = self.allocate(ident);
                if let Some(expr_pair) = pairs.next() {
                    let expr = self.expr_from_pair(expr_pair)?;
                    Ok(Expr::SetVariable(var_index, Box::new(expr)))
                } else {
                    Ok(Expr::GetVariable(var_index))
                }
            }
            Rule::expr_unary => {
                let mut pairs = pair.into_inner();
                let op = pairs.next().unwrap().as_str();
                let inner = self.expr_from_pair(pairs.next().unwrap())?;
                Ok(match op {
                    "+" => inner,
                    "-" => Expr::Function {
                        name: Function::Neg,
                        args: vec![inner],
                    },
                    _ => unreachable!(),
                })
            }
            Rule::expr_interval => {
                let mut pairs = pair.into_inner();
                let multiple = self.expr_from_pair(pairs.next().unwrap())?;
                let interval_unit = match pairs.next().unwrap().as_rule() {
                    Rule::interval_unit_week => 604_800_000_000,
                    Rule::interval_unit_day => 86_400_000_000,
                    Rule::interval_unit_hour => 3_600_000_000,
                    Rule::interval_unit_minute => 60_000_000,
                    Rule::interval_unit_second => 1_000_000,
                    Rule::interval_unit_ms => 1_000,
                    Rule::interval_unit_us => 1,
                    _ => unreachable!(),
                };
                Ok(Expr::Function {
                    name: Function::Mul,
                    args: vec![multiple, Expr::Value(Value::Interval(interval_unit))],
                })
            }
            rule => {
                let name = match rule {
                    Rule::expr_case_value_when => Function::CaseValueWhen,
                    Rule::expr_and => Function::And,
                    Rule::expr_or => Function::Or,
                    Rule::expr_not => Function::Not,
                    Rule::expr_timestamp => Function::Timestamp,
                    r => panic!("unexpected rule <{:?}> while parsing an expression", r),
                };
                let args = self.expr_from_pairs(pair.into_inner())?;
                Ok(Expr::Function { name, args })
            }
        }
    }
}

fn parse_number(input: &str) -> Result<Value, Error> {
    match input.get(..2) {
        Some("0x") | Some("0X") => {
            let number =
                u64::from_str_radix(&input[2..], 16).with_context(|_| ErrorKind::IntegerOverflow(input.to_owned()))?;
            return Ok(number.into());
        }
        _ => {}
    }

    Ok(match input.parse::<u64>() {
        Ok(number) => number.into(),
        Err(_) => input.parse::<f64>().unwrap().into(),
    })
}

macro_rules! define_function {
    (
        pub enum $F:ident {
        'function:
            $($fi:ident = $fs:tt,)*
        'sym_op:
            $($si:ident = $ss:tt,)*
        'named_op:
            $($ni:ident = $ns:tt,)*
        'else:
            $($ei:ident = $es:tt,)*
        }
    ) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum $F {
            $($fi,)+
            $($si,)+
            $($ni,)+
            $($ei,)+
        }

        impl fmt::Display for $F {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(match self {
                    $($F::$fi => $fs,)*
                    $($F::$si => $ss,)*
                    $($F::$ni => $ns,)*
                    $($F::$ei => $es,)*
                })
            }
        }

        impl $F {
            fn from_name(name: String) -> Result<Self, Error> {
                Ok(match &*name {
                    $($fs => $F::$fi,)*
                    _ => return Err(ErrorKind::UnknownFunction(name).into()),
                })
            }

            fn from_op(name: &str) -> Option<Self> {
                match name {
                    $($ss => Some($F::$si),)*
                    _ => {
                        $(if name.eq_ignore_ascii_case($ns) {
                            return Some($F::$ni);
                        })*
                        None
                    }
                }
            }
        }
    }
}

define_function! {
    pub enum Function {
    'function:
        RandRegex = "rand.regex",
        RandRange = "rand.range",
        RandRangeInclusive = "rand.range_inclusive",
        RandUniform = "rand.uniform",
        RandUniformInclusive = "rand.uniform_inclusive",
        RandZipf = "rand.zipf",
        RandLogNormal = "rand.log_normal",
        RandBool = "rand.bool",

    'sym_op:
        Eq = "=",
        Lt = "<",
        Gt = ">",
        Le = "<=",
        Ge = ">=",
        Ne = "<>",
        Add = "+",
        Sub = "-",
        Mul = "*",
        FloatDiv = "/",
        Concat = "||",

    'named_op:
        Is = "is",
        IsNot = "is not",
        Or = "or",
        And = "and",
        Not = "not",

    'else:
        Neg = "unary -",
        CaseValueWhen = "case ... when",
        Timestamp = "timestamp",
    }
}
