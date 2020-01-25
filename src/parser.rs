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
    table_name_index: usize,
    qualified_name: String,
    unique_name: String,
}

impl QName {
    /// Creates a new qualified name from its components.
    pub fn new(database: Option<&str>, schema: Option<&str>, table: &str) -> Self {
        let estimated_joined_len =
            database.map_or(0, |s| s.len() + 1) + schema.map_or(0, |s| s.len() + 1) + table.len();

        let mut qualified_name = String::with_capacity(estimated_joined_len);
        let mut unique_name = String::with_capacity(estimated_joined_len);
        if let Some(db) = database {
            qualified_name.push_str(db);
            qualified_name.push('.');
            unescape_into(&mut unique_name, db, true);
            unique_name.push('.');
        }
        if let Some(schema) = schema {
            qualified_name.push_str(schema);
            qualified_name.push('.');
            unescape_into(&mut unique_name, schema, true);
            unique_name.push('.');
        }
        let table_name_index = qualified_name.len();
        qualified_name.push_str(table);
        unescape_into(&mut unique_name, table, true);

        Self {
            table_name_index,
            qualified_name,
            unique_name,
        }
    }

    fn from_pairs(pairs: Pairs<'_, Rule>, override_schema: [Option<&str>; 2]) -> Self {
        let (mut database, mut schema, mut table) = (None, None, None);
        for pair in pairs {
            database = schema;
            schema = table;
            table = Some(pair.as_str());
        }
        if override_schema[1].is_some() {
            database = override_schema[0];
            schema = override_schema[1];
        }
        Self::new(database, schema, table.expect("at least one name"))
    }

    /// Parses a qualified name
    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut pairs = TemplateParser::parse(Rule::qname, input)?;
        Ok(Self::from_pairs(pairs.next().unwrap().into_inner(), [None; 2]))
    }

    /// Obtains the table name.
    ///
    /// When `qualified` is true, returns the qualified name connected with dots
    /// (`"db"."schema"."table"`). Otherwise just returns the unqualified name.
    pub fn table_name(&self, qualified: bool) -> &str {
        if qualified {
            &self.qualified_name
        } else {
            &self.qualified_name[self.table_name_index..]
        }
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
    pub fn unique_name(&self) -> &str {
        &self.unique_name
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

/// One single table.
#[derive(Debug, Clone, Default)]
pub struct Table {
    /// The default table name.
    pub name: QName,

    /// The content of the CREATE TABLE statement.
    pub content: String,

    /// The expressions to populate the table.
    pub exprs: Vec<Expr>,

    /// The indices of the derived tables, and the number of rows to generate.
    pub derived: Vec<(usize, Expr)>,
}

/// A parsed template.
#[derive(Debug, Clone, Default)]
pub struct Template {
    /// The expressions shared among all tables and rows.
    ///
    /// These should be evaluated only once.
    pub global_exprs: Vec<Expr>,

    /// Number of variables involved in the expressions (including globals).
    pub variables_count: usize,

    /// The tables to be written out.
    pub tables: Vec<Table>,
}

/// A parsed expression.
#[derive(Debug, Clone)]
pub enum Expr {
    /// The `rownum` symbol.
    RowNum,
    /// The `subrownum` symbol.
    SubRowNum,
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
    pub fn parse(input: &str, init_globals: &[String], override_schema: Option<&str>) -> Result<Self, Error> {
        let mut alloc = Allocator::default();
        if let Some(schema) = override_schema {
            alloc.set_schema_name(schema)?;
        }
        let mut template = Self::default();

        template.global_exprs = init_globals
            .iter()
            .map(|init_global_input| {
                let pairs = TemplateParser::parse(Rule::stmt, init_global_input)?;
                alloc.stmt_from_pairs(pairs)
            })
            .collect::<Result<_, _>>()?;

        let pairs = TemplateParser::parse(Rule::create_table, input)?;
        let mut table_map = HashMap::new();
        let mut expected_child_name = None::<QName>;

        for pair in pairs {
            match pair.as_rule() {
                Rule::EOI => {}
                Rule::stmt => template
                    .global_exprs
                    .push(alloc.expr_binary_from_pairs(pair.into_inner())?),
                Rule::single_table => {
                    let table = alloc.table_from_pairs(pair.into_inner())?;
                    let table_name = table.name.unique_name();
                    if let Some(child_name) = &expected_child_name {
                        if child_name.unique_name() != &*table_name {
                            return Err(Error::DerivedTableNameMismatch {
                                for_each_row: child_name.table_name(true).to_owned(),
                                create_table: table.name.table_name(true).to_owned(),
                            });
                        }
                    }
                    table_map.insert(table_name.to_owned(), template.tables.len());
                    template.tables.push(table);
                }
                Rule::dependency_directive => {
                    // register the next table as derived from the specified parent table.
                    let child_index = template.tables.len();
                    let (parent, child, count) = alloc.dependency_directive_from_pairs(pair.into_inner())?;
                    if let Some(parent_index) = table_map.get(parent.unique_name()) {
                        template.tables[*parent_index].derived.push((child_index, count));
                        expected_child_name = Some(child);
                    } else {
                        return Err(Error::UnknownParentTable {
                            parent: parent.table_name(true).to_owned(),
                        });
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        template.variables_count = alloc.map.len();
        Ok(template)
    }
}

/// Local variable allocator. This structure keeps record of local variables `@x` and assigns a
/// unique number of each variable, so that they can be referred using a number instead of a string.
#[derive(Default)]
struct Allocator<'a> {
    override_schema: [Option<&'a str>; 2],
    map: HashMap<String, usize>,
}

impl<'a> Allocator<'a> {
    fn set_schema_name(&mut self, schema: &'a str) -> Result<(), Error> {
        let pairs = TemplateParser::parse(Rule::qname, schema)?;
        for pair in pairs {
            self.override_schema = [self.override_schema[1], Some(pair.as_str())];
        }
        Ok(())
    }

    /// Allocates a local variable index given the name.
    fn allocate(&mut self, raw_var_name: &str) -> usize {
        let mut var_name = String::with_capacity(raw_var_name.len());
        unescape_into(&mut var_name, raw_var_name, false);
        let count = self.map.len();
        *self.map.entry(var_name).or_insert(count)
    }

    /// Creates a single table.
    fn table_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Table, Error> {
        let mut table = Table::default();

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_create | Rule::kw_table => {}
                Rule::qname => table.name = QName::from_pairs(pair.into_inner(), self.override_schema),
                Rule::open_paren | Rule::close_paren | Rule::any_text => {
                    let s = pair.as_str();
                    // insert a space if needed to ensure word boundaries
                    if table.content.ends_with(is_ident_char) && s.starts_with(is_ident_char) {
                        table.content.push(' ');
                    }
                    table.content.push_str(s);
                }
                Rule::stmt => table.exprs.push(self.expr_binary_from_pairs(pair.into_inner())?),
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok(table)
    }

    /// Parses a dependency directive.
    fn dependency_directive_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<(QName, QName, Expr), Error> {
        let mut parent = QName::default();
        let mut child = QName::default();
        let mut count = Expr::default();
        let mut is_parent = true;

        for pair in pairs {
            match pair.as_rule() {
                Rule::kw_for | Rule::kw_each | Rule::kw_rows | Rule::kw_of | Rule::kw_generate => {}
                Rule::expr => count = self.expr_from_pairs(pair.into_inner())?,
                Rule::qname => {
                    let target = if is_parent {
                        is_parent = false;
                        &mut parent
                    } else {
                        &mut child
                    };
                    *target = QName::from_pairs(pair.into_inner(), self.override_schema);
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        Ok((parent, child, count))
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
                Rule::expr_bit_or | Rule::expr_bit_and | Rule::expr_and | Rule::expr_add | Rule::expr_mul => {
                    args.push(self.expr_binary_from_pairs(pair.into_inner())?)
                }
                Rule::expr_not => args.push(self.expr_not_from_pairs(pair.into_inner())?),
                Rule::expr_unary => args.push(self.expr_unary_from_pairs(pair.into_inner())?),
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
                | Rule::op_bit_and
                | Rule::op_bit_or
                | Rule::op_bit_xor
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
            Rule::kw_subrownum => Expr::SubRowNum,
            Rule::kw_current_timestamp => Expr::CurrentTimestamp,
            Rule::kw_null => Expr::Value(Value::Null),
            Rule::kw_true => Expr::Value(1_u64.into()),
            Rule::kw_false => Expr::Value(0_u64.into()),
            Rule::expr_group => self.expr_group_from_pairs(pair.into_inner())?,
            Rule::number => Expr::Value(parse_number(pair.as_str())?),
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
                    let q_name = QName::from_pairs(pair.into_inner(), [None; 2]);
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

    /// Creates any expression involving a unary operator `+x`, `-x`, `x[i]`, etc.
    fn expr_unary_from_pairs(&mut self, pairs: Pairs<'_, Rule>) -> Result<Expr, Error> {
        let mut op_stack = Vec::<&dyn Function>::new();
        let mut base = Expr::default();
        for pair in pairs {
            match pair.as_rule() {
                Rule::op_add => {}
                Rule::op_sub => op_stack.push(&functions::ops::Neg),
                Rule::op_bit_not => op_stack.push(&functions::ops::BitNot),
                Rule::expr_primary => {
                    base = self.expr_primary_from_pairs(pair.into_inner())?;
                }
                Rule::expr => {
                    base = Expr::Function {
                        function: &functions::array::Subscript,
                        args: vec![base, self.expr_from_pairs(pair.into_inner())?],
                    }
                }
                r => unreachable!("Unexpected rule {:?}", r),
            }
        }

        for function in op_stack.into_iter().rev() {
            base = Expr::Function {
                function,
                args: vec![base],
            };
        }
        Ok(base)
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
fn function_from_name(name: &str) -> Result<&'static dyn Function, Error> {
    use functions::{
        array, ops, rand,
        string::{self, Unit},
    };

    Ok(match name {
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
        "rand.shuffle" => &rand::Shuffle,
        "rand.uuid" => &rand::Uuid,
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
        "generate_series" => &array::GenerateSeries,
        _ => return Err(Error::UnknownFunction(name.to_owned())),
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
        Rule::op_bit_and => &functions::ops::Bitwise::And,
        Rule::op_bit_or => &functions::ops::Bitwise::Or,
        Rule::op_bit_xor => &functions::ops::Bitwise::Xor,
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
        "create table a (); {{ 1 }}",
        "create table a (); {{ 1 }} create table b ();",
        "create table a (); {{ for each row of x generate 1 row of b }} create table b ();",
        "create table a (); {{ for each row of a generate 1 row of c }} create table b ();",
        "create table a (); {{ for each row of b generate 1 row of a }} create table b ();",
        "create table a (); {{ for each row of a generate (*) rows of b }} create table b ();",
    ];
    for tc in &test_cases {
        let res = Template::parse(tc, &[], None);
        assert!(res.is_err(), "unexpected for case {}:\n{:#?}", tc, res);
    }
}
