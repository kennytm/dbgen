//! Template parser.

pub(crate) use self::derived::Rule;
use self::derived::TemplateParser;
use crate::{
    error::Error,
    functions::{self, Function},
    value::Value,
};

use pest::{iterators::Pairs, Parser};
use std::{cmp::Ordering, collections::HashMap, mem};

mod derived {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "parser.pest"]
    pub(super) struct TemplateParser;
}

/// A schema-qualified name with quotation marks still intact.
#[derive(Debug, Clone, Default)]
pub struct QName {
    /// Database name
    pub database: Option<String>,
    /// Schema name
    pub schema: Option<String>,
    /// Table name
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
        let mut pairs = TemplateParser::parse(Rule::qname, input)?;
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
    ///     multiple characters (e.g. `İ` → `i̇`), only the first (`i`) will be
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
                    write!(res, "%{:02X}", u32::from(c)).unwrap();
                    continue;
                }
            }
            _ => {}
        }
        res.push(c);
    }
}

/// A parsed template.
#[derive(Debug, Clone)]
pub struct Template {
    /// The default table name.
    pub name: QName,

    /// The content of the CREATE TABLE statement.
    pub content: String,

    /// The expressions to populate the table.
    pub exprs: Vec<Expr>,

    /// The expressions shared among all rows.
    ///
    /// These should be evaluated only once.
    pub global_exprs: Vec<Expr>,

    /// Number of variables involved in the expressions (including globals).
    pub variables_count: usize,
}

/// A parsed expression.
#[derive(Debug, Clone)]
pub enum Expr {
    /// The `rownum` symbol.
    RowNum,
    /// The `current_timestamp` symbol.
    CurrentTimestamp,
    /// A constant value.
    Value(Value),
    /// Symbol of a local variable `@x`.
    GetVariable(usize),
    /// A variable assignment expression `@x := y`.
    SetVariable(usize, Box<Expr>),
    /// A function call.
    Function {
        /// The function.
        function: &'static dyn Function,
        /// Function arguments.
        args: Vec<Expr>,
    },
    /// A `CASE … WHEN` expression.
    CaseValueWhen {
        /// The expression to match against.
        value: Option<Box<Expr>>,
        /// The conditions and their corresponding results.
        conditions: Vec<(Expr, Expr)>,
        /// The result when all conditions failed.
        otherwise: Option<Box<Expr>>,
    },
}

impl Default for Expr {
    fn default() -> Self {
        Self::Value(Value::Null)
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Template {
    /// Parses a raw string into a structured template.
    pub fn parse(input: &str, init_globals: &[String]) -> Result<Self, Error> {
        let mut alloc = Allocator::default();
        let mut global_exprs = init_globals
            .iter()
            .map(|init_global_input| {
                let pairs = TemplateParser::parse(Rule::stmt, init_global_input)?;
                alloc.stmt_from_pairs(pairs)
            })
            .collect::<Result<_, _>>()?;

        let pairs = TemplateParser::parse(Rule::create_table, input)?;

        let mut name = QName::default();
        let mut exprs = Vec::new();
        let mut content = String::from("(");
        let mut is_global = true;

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_create | Rule::kw_table => is_global = false,
                Rule::qname => name = QName::from_pairs(pair.into_inner()),
                Rule::column_definition | Rule::table_options => {
                    let s = pair.as_str();
                    // insert a space if needed to ensure word boundaries
                    if content.ends_with(is_ident_char) && s.starts_with(is_ident_char) {
                        content.push(' ');
                    }
                    content.push_str(s);
                }
                Rule::stmt => if is_global { &mut global_exprs } else { &mut exprs }
                    .push(alloc.expr_binary_from_pairs(pair.into_inner())?),
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Self {
            name,
            content,
            exprs,
            global_exprs,
            variables_count: alloc.map.len(),
        })
    }
}

/// Local variable allocator. This structure keeps record of local variables `@x` and assigns a
/// unique number of each variable, so that they can be referred using a number instead of a string.
#[derive(Default)]
struct Allocator {
    map: HashMap<String, usize>,
}

impl Allocator {
    /// Allocates a local variable index given the name.
    fn allocate(&mut self, raw_var_name: &str) -> usize {
        let mut var_name = String::with_capacity(raw_var_name.len());
        unescape_into(&mut var_name, raw_var_name, false);
        let count = self.map.len();
        *self.map.entry(var_name).or_insert(count)
    }

    /// Creates a statement expression `a; b; c`.
    fn stmt_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        self.expr_binary_from_pairs(pairs.next().unwrap().into_inner())
    }

    /// Creates an assignment expression `@x := @y := z`.
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

    /// Creates any expression involving a binary operator `x + y`, `x * y`, etc.
    fn expr_binary_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut args = Vec::with_capacity(1);
        let mut op = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::expr_and | Rule::expr_add | Rule::expr_mul => {
                    args.push(self.expr_binary_from_pairs(pair.into_inner())?)
                }
                Rule::expr_not => args.push(self.expr_not_from_pairs(pair.into_inner())?),
                Rule::expr_subscript => args.push(self.expr_subscript_from_pairs(pair.into_inner())?),
                Rule::expr => args.push(self.expr_from_pairs(pair.into_inner())?),
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
                | Rule::op_float_div
                | Rule::op_semicolon => {
                    match op {
                        Some(o) if o != rule => {
                            args = vec![Expr::Function {
                                function: function_from_rule(o),
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
                function: function_from_rule(o),
                args,
            }
        } else {
            debug_assert_eq!(args.len(), 1);
            args.swap_remove(0)
        })
    }

    /// Creates an array subscript expression `p[i][j][k]`
    fn expr_subscript_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut base = Expr::default();

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::expr_primary => base = self.expr_primary_from_pairs(pair.into_inner())?,
                Rule::expr => {
                    base = Expr::Function {
                        function: &functions::array::Subscript,
                        args: vec![base, self.expr_from_pairs(pair.into_inner())?],
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(base)
    }

    /// Creates a NOT expression `NOT NOT NOT x`.
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
                            function: &functions::ops::Not,
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

    /// Creates a primary expression.
    fn expr_primary_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let pair = pairs.next().unwrap();
        Ok(match pair.as_rule() {
            Rule::kw_rownum => Expr::RowNum,
            Rule::kw_current_timestamp => Expr::CurrentTimestamp,
            Rule::kw_null => Expr::Value(Value::Null),
            Rule::kw_true => Expr::Value(1_u64.into()),
            Rule::kw_false => Expr::Value(0_u64.into()),
            Rule::expr_group => self.expr_group_from_pairs(pair.into_inner())?,
            Rule::number => Expr::Value(parse_number(pair.as_str())?),
            Rule::expr_unary => self.expr_unary_from_pairs(pair.into_inner())?,
            Rule::expr_timestamp => self.expr_timestamp_from_pairs(pair.into_inner())?,
            Rule::expr_interval => self.expr_interval_from_pairs(pair.into_inner())?,
            Rule::expr_get_variable => self.expr_get_variable_from_pairs(pair.into_inner())?,
            Rule::expr_array => self.expr_array_from_pairs(pair.into_inner())?,
            Rule::expr_function => self.expr_function_from_pairs(pair.into_inner())?,
            Rule::expr_substring_function => self.expr_substring_from_pairs(pair.into_inner())?,
            Rule::expr_overlay_function => self.expr_overlay_from_pairs(pair.into_inner())?,
            Rule::expr_case_value_when => self.expr_case_value_when_from_pairs(pair.into_inner())?,

            Rule::single_quoted => {
                let mut string = String::with_capacity(pair.as_str().len());
                unescape_into(&mut string, pair.as_str(), false);
                Expr::Value(string.into())
            }

            r => unreachable!("Unexpected rule {:?}", r),
        })
    }

    /// Creates a function call expression `x.y.z(a, b, c)`.
    fn expr_function_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut function: &dyn Function = &functions::ops::Last;
        let mut args = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::qname => {
                    let q_name = QName::from_pairs(pair.into_inner());
                    function = function_from_name(q_name.unique_name())?;
                }
                Rule::expr => args.push(self.expr_from_pairs(pair.into_inner())?),
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::Function { function, args })
    }

    /// Creates an array expression `ARRAY[a, b, c]`.
    fn expr_array_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut args = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_array => {}
                Rule::expr => args.push(self.expr_from_pairs(pair.into_inner())?),
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::Function {
            function: &functions::array::Array,
            args,
        })
    }

    /// Creates a group expression `(x)`.
    fn expr_group_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        self.expr_from_pairs(pairs.next().unwrap().into_inner())
    }

    /// Creates a local variable expression `@x`.
    fn expr_get_variable_from_pairs(&mut self, mut pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let pair = pairs.next().unwrap();
        Ok(Expr::GetVariable(self.allocate(pair.as_str())))
    }

    /// Creates any expression involving a unary operator `+x`, `-x`, etc.
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
                            function: &functions::ops::Neg,
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

    /// Creates a `CASE … WHEN` expression.
    fn expr_case_value_when_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut value = None;
        let mut pattern = Expr::default();
        let mut conditions = Vec::with_capacity(2);
        let mut otherwise = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::kw_case | Rule::kw_when | Rule::kw_then | Rule::kw_else | Rule::kw_end => {}
                Rule::case_value_when_value | Rule::case_value_when_pattern => {
                    let expr = self.expr_group_from_pairs(pair.into_inner())?;
                    match rule {
                        Rule::case_value_when_value => value = Some(Box::new(expr)),
                        Rule::case_value_when_pattern => pattern = expr,
                        _ => unreachable!(),
                    }
                }
                Rule::case_value_when_result | Rule::case_value_when_else => {
                    let expr = self.stmt_from_pairs(pair.into_inner())?;
                    match rule {
                        Rule::case_value_when_result => conditions.push((mem::take(&mut pattern), expr)),
                        Rule::case_value_when_else => otherwise = Some(Box::new(expr)),
                        _ => unreachable!(),
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(Expr::CaseValueWhen {
            value,
            conditions,
            otherwise,
        })
    }

    /// Creates a `TIMESTAMP` expression.
    fn expr_timestamp_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut function: &dyn Function = &functions::time::Timestamp;
        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_timestamp => {}
                Rule::kw_with | Rule::kw_time | Rule::kw_zone => {
                    function = &functions::time::TimestampWithTimeZone;
                }
                Rule::expr_primary => {
                    return Ok(Expr::Function {
                        function,
                        args: vec![self.expr_primary_from_pairs(pair.into_inner())?],
                    });
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        unreachable!("Pairs exhausted without finding the inner expression");
    }

    /// Creates an `INTERVAL` expression.
    fn expr_interval_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut unit = 1;
        let mut expr = Expr::default();

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_interval => {}
                Rule::expr => expr = self.expr_from_pairs(pair.into_inner())?,
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
            function: &functions::ops::Arith::Mul,
            args: vec![expr, Expr::Value(Value::Interval(unit))],
        })
    }

    /// Creates a `substring` function expression.
    fn expr_substring_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        use functions::string::{Substring, Unit};

        let mut function = &Substring(Unit::Characters);
        let mut input = Expr::default();
        let mut from = None;
        let mut length = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::kw_substring | Rule::kw_from | Rule::kw_for | Rule::kw_using => {}
                Rule::kw_octets => function = &Substring(Unit::Octets),
                Rule::kw_characters => function = &Substring(Unit::Characters),
                Rule::substring_input | Rule::substring_from | Rule::substring_for => {
                    let expr = self.expr_group_from_pairs(pair.into_inner())?;
                    match rule {
                        Rule::substring_input => input = expr,
                        Rule::substring_from => from = Some(expr),
                        Rule::substring_for => length = Some(expr),
                        _ => unreachable!(),
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        let mut args = vec![input, from.unwrap_or_else(|| Expr::Value(1.into()))];
        if let Some(length) = length {
            args.push(length);
        }
        Ok(Expr::Function { function, args })
    }

    /// Creates an `overlay` function expression.
    fn expr_overlay_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        use functions::string::{Overlay, Unit};

        let mut function = &Overlay(Unit::Characters);
        let mut input = Expr::default();
        let mut placing = Expr::default();
        let mut from = Expr::default();
        let mut length = None;

        for pair in pairs {
            let rule = pair.as_rule();
            match rule {
                Rule::kw_overlay | Rule::kw_placing | Rule::kw_from | Rule::kw_for | Rule::kw_using => {}
                Rule::kw_octets => function = &Overlay(Unit::Octets),
                Rule::kw_characters => function = &Overlay(Unit::Characters),
                Rule::substring_input | Rule::substring_from | Rule::substring_for | Rule::overlay_placing => {
                    let expr = self.expr_group_from_pairs(pair.into_inner())?;
                    match rule {
                        Rule::substring_input => input = expr,
                        Rule::substring_from => from = expr,
                        Rule::substring_for => length = Some(expr),
                        Rule::overlay_placing => placing = expr,
                        _ => unreachable!(),
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        let mut args = vec![input, placing, from];
        if let Some(length) = length {
            args.push(length);
        }
        Ok(Expr::Function { function, args })
    }
}

/// Parses a number (integer or floating-point number) into a value.
fn parse_number(input: &str) -> Result<Value, Error> {
    match input.get(..2) {
        Some("0x") | Some("0X") => {
            let number = u64::from_str_radix(&input[2..], 16).map_err(|_| Error::IntegerOverflow(input.to_owned()))?;
            return Ok(number.into());
        }
        _ => {}
    }

    Ok(match input.parse::<u64>() {
        Ok(number) => number.into(),
        Err(_) => input.parse::<f64>().unwrap().into(),
    })
}

/// Obtains a function from its name.
fn function_from_name(name: String) -> Result<&'static dyn Function, Error> {
    use functions::{
        ops, rand,
        string::{self, Unit},
    };

    Ok(match &*name {
        "rand.regex" => &rand::Regex,
        "rand.range" => &rand::Range,
        "rand.range_inclusive" => &rand::RangeInclusive,
        "rand.uniform" => &rand::Uniform,
        "rand.uniform_inclusive" => &rand::UniformInclusive,
        "rand.zipf" => &rand::Zipf,
        "rand.log_normal" => &rand::LogNormal,
        "rand.bool" => &rand::Bool,
        "rand.finite_f32" => &rand::FiniteF32,
        "rand.finite_f64" => &rand::FiniteF64,
        "rand.u31_timestamp" => &rand::U31Timestamp,
        "greatest" => &ops::Extremum {
            order: Ordering::Greater,
        },
        "least" => &ops::Extremum { order: Ordering::Less },
        "round" => &ops::Round,
        "div" => &ops::Div,
        "mod" => &ops::Mod,
        "char_length" | "character_length" => &string::Length(Unit::Characters),
        "octet_length" => &string::Length(Unit::Octets),
        "coalesce" => &ops::Coalesce,
        _ => return Err(Error::UnknownFunction(name)),
    })
}

/// Obtains a function from the parser rule.
fn function_from_rule(rule: Rule) -> &'static dyn Function {
    match rule {
        Rule::op_lt => &functions::ops::Compare {
            lt: true,
            eq: false,
            gt: false,
        },
        Rule::op_eq => &functions::ops::Compare {
            lt: false,
            eq: true,
            gt: false,
        },
        Rule::op_gt => &functions::ops::Compare {
            lt: false,
            eq: false,
            gt: true,
        },
        Rule::op_le => &functions::ops::Compare {
            lt: true,
            eq: true,
            gt: false,
        },
        Rule::op_ne => &functions::ops::Compare {
            lt: true,
            eq: false,
            gt: true,
        },
        Rule::op_ge => &functions::ops::Compare {
            lt: false,
            eq: true,
            gt: true,
        },
        Rule::op_add => &functions::ops::Arith::Add,
        Rule::op_sub => &functions::ops::Arith::Sub,
        Rule::op_mul => &functions::ops::Arith::Mul,
        Rule::op_float_div => &functions::ops::Arith::FloatDiv,
        Rule::op_semicolon => &functions::ops::Last,
        Rule::op_concat => &functions::string::Concat,
        Rule::kw_is => &functions::ops::Identical { eq: true },
        Rule::is_not => &functions::ops::Identical { eq: false },
        Rule::kw_and => &functions::ops::Logic { identity: true },
        Rule::kw_or => &functions::ops::Logic { identity: false },
        r => unreachable!("Unexpected operator rule {:?}", r),
    }
}

#[test]
fn test_parse_template_error() {
    let test_cases = [
        "create table a ({{ 4 = 4 = 4 }});",
        "create table a ({{ 4 is 4 is 4 }});",
        "create table a ({{ 4 <> 4 <> 4 }});",
        "create table a ({{ 4 is not 4 is not 4 }});",
        "create table a ({{ 4 < 4 < 4 }});",
        "create table a ({{ 4 <= 4 <= 4 }});",
        "create table a ({{ 4 > 4 > 4 }});",
        "create table a ({{ 4 >= 4 >= 4 }});",
    ];
    for tc in &test_cases {
        let res = Template::parse(tc, &[]);
        assert!(res.is_err(), "unexpected for case {}:\n{:#?}", tc, res);
    }
}
