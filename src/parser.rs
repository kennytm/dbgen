use crate::{
    error::{Error, ErrorKind},
    value::Value,
};
use failure::ResultExt;
use pest::{iterators::Pairs, Parser};
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
    fn from_pairs(pairs: Pairs<'_, Rule>) -> Self {
        let mut database = None;
        let mut schema = None;
        let mut table = None;
        for pair in pairs {
            database = schema;
            schema = table;
            table = Some(pair.as_str().to_owned());
        }
        Self {
            database,
            schema,
            table: table.expect("at least one name"),
        }
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
    Function {
        name: Function,
        args: Vec<Expr>,
    },
    CaseValueWhen {
        value: Box<Expr>,
        conditions: Vec<(Expr, Expr)>,
        otherwise: Option<Box<Expr>>,
    },
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let pairs = TemplateParser::parse(Rule::create_table, input).context(ErrorKind::ParseTemplate)?;

        let mut name = None;
        let mut alloc = Allocator::default();
        let mut exprs = Vec::new();
        let mut content = String::from("(");

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_create | Rule::kw_table => {}
                Rule::qname => {
                    name = Some(QName::from_pairs(pair.into_inner()));
                }
                Rule::column_definition | Rule::table_options => {
                    content.push_str(pair.as_str());
                }
                Rule::expr => {
                    exprs.push(alloc.expr_from_pairs(pair.into_inner())?);
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Self {
            name: name.unwrap(),
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
    fn allocate(&mut self, raw_var_name: &str) -> usize {
        let mut var_name = String::with_capacity(raw_var_name.len());
        unescape_into(&mut var_name, raw_var_name, false);
        let count = &mut self.count;
        *self.map.entry(var_name).or_insert_with(|| {
            let last = *count;
            *count += 1;
            last
        })
    }

    fn expr_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut indices = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::ident => {
                    indices.push(self.allocate(pair.as_str()));
                }
                Rule::expr_or => {
                    let mut expr = self.expr_binary_from_pairs(pair.into_inner())?;
                    for i in indices.into_iter().rev() {
                        expr = Expr::SetVariable(i, Box::new(expr));
                    }
                    return Ok(expr);
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        unreachable!("Pairs exhausted without finding the inner expression");
    }

    fn expr_binary_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut args = Vec::with_capacity(1);
        let mut op = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::expr_and | Rule::expr_add | Rule::expr_mul => {
                    args.push(self.expr_binary_from_pairs(pair.into_inner())?);
                }
                Rule::expr_not => {
                    args.push(self.expr_not_from_pairs(pair.into_inner())?);
                }
                Rule::expr_primary => {
                    args.push(self.expr_primary_from_pairs(pair.into_inner())?);
                }
                Rule::kw_or
                | Rule::kw_and
                | Rule::is_not
                | Rule::kw_is
                | Rule::op_le
                | Rule::op_ge
                | Rule::op_lt
                | Rule::op_gt
                | Rule::op_eq
                | Rule::op_ne
                | Rule::op_add
                | Rule::op_sub
                | Rule::op_concat
                | Rule::op_mul
                | Rule::op_float_div => {
                    match op {
                        Some(o) if o != rule => {
                            args = vec![Expr::Function {
                                name: Function::from_rule(o),
                                args,
                            }];
                        }
                        _ => {}
                    }
                    op = Some(rule);
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(if let Some(o) = op {
            Expr::Function {
                name: Function::from_rule(o),
                args,
            }
        } else {
            debug_assert_eq!(args.len(), 1);
            args.swap_remove(0)
        })
    }

    fn expr_not_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut has_not = false;
        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_not => {
                    has_not = !has_not;
                }
                Rule::expr_cmp => {
                    let expr = self.expr_binary_from_pairs(pair.into_inner())?;
                    return Ok(if has_not {
                        Expr::Function {
                            name: Function::Not,
                            args: vec![expr],
                        }
                    } else {
                        expr
                    });
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }
        unreachable!("Pairs exhausted without finding the inner expression");
    }

    fn expr_primary_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let pair = pairs.next().unwrap();
        Ok(match pair.as_rule() {
            Rule::kw_rownum => Expr::RowNum,
            Rule::kw_null => Expr::Value(Value::Null),
            Rule::kw_true => Expr::Value(1_u64.into()),
            Rule::kw_false => Expr::Value(0_u64.into()),
            Rule::expr_group => self.expr_group_from_pairs(pair.into_inner())?,
            Rule::number => Expr::Value(parse_number(pair.as_str())?),
            Rule::expr_unary => self.expr_unary_from_pairs(pair.into_inner())?,
            Rule::expr_timestamp => self.expr_timestamp_from_pairs(pair.into_inner())?,
            Rule::expr_interval => self.expr_interval_from_pairs(pair.into_inner())?,
            Rule::expr_get_variable => self.expr_get_variable_from_pairs(pair.into_inner())?,
            Rule::expr_function => self.expr_function_from_pairs(pair.into_inner())?,
            Rule::expr_case_value_when => self.expr_case_value_when_from_pairs(pair.into_inner())?,

            Rule::single_quoted => {
                let mut string = String::with_capacity(pair.as_str().len());
                unescape_into(&mut string, pair.as_str(), false);
                Expr::Value(string.into())
            }

            r => unreachable!("Unexpected rule {:?}", r),
        })
    }

    fn expr_function_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut name = None;
        let mut args = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::qname => {
                    let q_name = QName::from_pairs(pair.into_inner());
                    name = Some(Function::from_name(q_name.unique_name())?);
                }
                Rule::expr => {
                    args.push(self.expr_from_pairs(pair.into_inner())?);
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::Function {
            name: name.unwrap(),
            args,
        })
    }

    fn expr_group_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        self.expr_from_pairs(pairs.next().unwrap().into_inner())
    }

    fn expr_get_variable_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let pair = pairs.next().unwrap();
        Ok(Expr::GetVariable(self.allocate(pair.as_str())))
    }

    fn expr_unary_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut has_neg = false;
        for pair in pairs {
            match pair.as_rule() {
                Rule::op_add => {}
                Rule::op_sub => {
                    has_neg = !has_neg;
                }
                Rule::expr_primary => {
                    let expr = self.expr_primary_from_pairs(pair.into_inner())?;
                    return Ok(if has_neg {
                        Expr::Function {
                            name: Function::Neg,
                            args: vec![expr],
                        }
                    } else {
                        expr
                    });
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }
        unreachable!("Pairs exhausted without finding the inner expression");
    }

    fn expr_case_value_when_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut value = None;
        let mut pattern = None;
        let mut conditions = Vec::with_capacity(2);
        let mut otherwise = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::kw_case | Rule::kw_when | Rule::kw_then | Rule::kw_else | Rule::kw_end => {}
                Rule::case_value_when_value
                | Rule::case_value_when_pattern
                | Rule::case_value_when_result
                | Rule::case_value_when_else => {
                    let expr = self.expr_group_from_pairs(pair.into_inner())?;
                    match rule {
                        Rule::case_value_when_value => value = Some(Box::new(expr)),
                        Rule::case_value_when_pattern => pattern = Some(expr),
                        Rule::case_value_when_result => conditions.push((pattern.take().unwrap(), expr)),
                        Rule::case_value_when_else => otherwise = Some(Box::new(expr)),
                        _ => unreachable!(),
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::CaseValueWhen {
            value: value.unwrap(),
            conditions,
            otherwise,
        })
    }

    fn expr_timestamp_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_timestamp => {}
                Rule::expr_primary => {
                    return Ok(Expr::Function {
                        name: Function::Timestamp,
                        args: vec![self.expr_primary_from_pairs(pair.into_inner())?],
                    })
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        unreachable!("Pairs exhausted without finding the inner expression");
    }

    fn expr_interval_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut unit = 1;
        let mut expr = None;

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_interval => {}
                Rule::expr => expr = Some(self.expr_from_pairs(pair.into_inner())?),
                Rule::kw_week => unit = 604_800_000_000,
                Rule::kw_day => unit = 86_400_000_000,
                Rule::kw_hour => unit = 3_600_000_000,
                Rule::kw_minute => unit = 60_000_000,
                Rule::kw_second => unit = 1_000_000,
                Rule::kw_millisecond => unit = 1_000,
                Rule::kw_microsecond => unit = 1,
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::Function {
            name: Function::Mul,
            args: vec![expr.unwrap(), Expr::Value(Value::Interval(unit))],
        })
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
        'rule:
            $($ri:ident = $rs:tt / $rr:ident,)*
        'else:
            $($ei:ident = $es:tt,)*
        }
    ) => {
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum $F {
            $($fi,)+
            $($ri,)+
            $($ei,)+
        }

        impl fmt::Display for $F {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(match self {
                    $($F::$fi => $fs,)*
                    $($F::$ri => $rs,)*
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

            fn from_rule(rule: Rule) -> Self {
                match rule {
                    $(Rule::$rr => $F::$ri,)*
                    r => unreachable!("Unexpected operator rule {:?}", r),
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

        Greatest = "greatest",
        Least = "least",

    'rule:
        Eq = "=" / op_eq,
        Lt = "<" / op_lt,
        Gt = ">" / op_gt,
        Le = "<=" / op_le,
        Ge = ">=" / op_ge,
        Ne = "<>" / op_ne,
        Add = "+" / op_add,
        Sub = "-" / op_sub,
        Mul = "*" / op_mul,
        FloatDiv = "/" / op_float_div,
        Concat = "||" / op_concat,
        Is = "is" / kw_is,
        IsNot = "is not" / is_not,
        Or = "or" / kw_or,
        And = "and" / kw_and,

    'else:
        Not = "not",
        Neg = "neg",
        Timestamp = "timestamp",
    }
}
