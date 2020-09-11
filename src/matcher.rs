//! 消息匹配实现。

use phf::phf_map;
use std::str::FromStr;
use strum_macros::{EnumString, ToString};

use super::error::Error;
use super::models::Message;
use super::result::Result;

pub type Groups = Vec<Vec<Cont>>;

pub static FIELD_OPERATORS: phf::Map<&'static str, &'static [Operator]> = phf_map! {
    "message.text" =>  &[Operator::Eq, Operator::ContainsOne, Operator::ContainsAll],
    "message.from.is_bot" => &[]
};

/// 匹配器。一般作为表达式的编译目标。
///
/// 匹配器可表达与字符串规则完全对应的结构化的条件关系。
/// 每个匹配器对象都具备一个“条件组”序列。
/// ```
/// use matchingram::models::Message;
/// use matchingram::matcher::*;
///
/// // 手动创建一个匹配器对象：
/// let groups = vec![
///     vec![
///         Cont {
///             is_negative: false,
///             field: Field::MessageText,
///             operator: Some(Operator::ContainsOne),
///             value: Some(vec![Value::from_str("柬埔寨"), Value::from_str("东南亚")]),
///         },
///         Cont {
///             is_negative: false,
///             field: Field::MessageText,
///             operator: Some(Operator::ContainsOne),
///             value: Some(vec![Value::from_str("菠菜"), Value::from_str("博彩")]),
///         },
///     ],
///     vec![Cont {
///         is_negative: false,
///         field: Field::MessageText,
///         operator: Some(Operator::ContainsAll),
///         value: Some(vec![Value::from_str("承接"), Value::from_str("广告")]),
///     }],
/// ];
/// let mut matcher = Matcher::new(groups);
/// // 两条典型的东南亚博彩招人消息
/// let message_text1 = format!("柬埔寨菠菜需要的来");
/// let message_text2 = format!("东南亚博彩招聘");
/// // 一条业务宣传消息
/// let message_text3 = format!("承接博彩广告业务");
///
/// let message1 = Message {
///     text: Some(message_text1),
///     ..Default::default()
/// };
/// let message2 = Message {
///     text: Some(message_text2),
///     ..Default::default()
/// };
/// let message3 = Message {
///     text: Some(message_text3),
///     ..Default::default()
/// };
///
/// assert!(matcher.match_message(&message1)?);
/// assert!(matcher.match_message(&message2)?);
/// assert!(matcher.match_message(&message3)?);
/// # Ok::<(), matchingram::Error>(())
/// ```
/// 它对应的字符串表达式为：
/// ```text
/// (message.text contains_one {"柬埔寨" "东南亚"} and message.text contains_one {"菠菜" "博彩"}) or (message.text contains_all {"承接" "广告"})
/// ```
/// **注意**：匹配器中的所有条件之间都没有显式的关系存在，因为匹配器中每一个独立的组之间一定是 `or` 关系，组内的条件之间一定是 `and` 关系。即：已存在隐式的关系表达。
#[derive(Debug, Default)]
pub struct Matcher {
    /// 条件组序列。
    pub groups: Groups,
    // 上个组的匹配结果。
    is_last_match: bool,
}

impl Matcher {
    /// 解析规则表达式创建匹配器对象。
    /// 相比规则表达式匹配器对象具有更高的性能，因为不用再经历编译过程。为了提升性能，可将规则预编译为匹配器对象再执行匹配动作。
    pub fn from_rule<S: Into<String>>(rule: S) -> Result<Self> {
        use super::lexer::Lexer;
        use super::parser::Parser;

        let input = rule.into().chars().collect::<Vec<_>>();
        let mut lexer = Lexer::new(&input);
        let parser = Parser::new(&mut lexer)?;
        let matcher = parser.parse()?;

        Ok(matcher)
    }

    /// 使用条件组创建匹配器对象。
    pub fn new(groups: Groups) -> Self {
        Matcher {
            groups: groups,
            is_last_match: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Decimal(i64),
    Letter(String),
}

/// 单个条件。
#[derive(Debug)]
pub struct Cont {
    /// 是否取反。
    pub is_negative: bool,
    /// 字段。
    pub field: Field,
    /// 运算符。
    pub operator: Option<Operator>,
    /// 值。
    pub value: Option<Vec<Value>>,
}

/// 条件字段。
#[derive(Debug, Copy, Clone, EnumString, ToString)]
pub enum Field {
    /// 消息文本。
    #[strum(serialize = "message.text")]
    MessageText,
    /// 消息文本大小。
    #[strum(serialize = "message.text.size")]
    MessageTextSize,
    /// 消息来源是否为 bot。
    #[strum(serialize = "message.from.is_bot")]
    MessageFromIsBot,
}

/// 条件操作符。
#[derive(Debug, Eq, PartialEq, Copy, Clone, EnumString, ToString)]
#[strum(serialize_all = "snake_case")]
pub enum Operator {
    // 等于。
    Eq,
    // 大于。
    Gt,
    // 小于。
    Lt,
    // 大于或等于。
    Ge,
    // 小于或等于。
    Le,
    /// 包含其一。
    ContainsOne,
    /// 包含全部。
    ContainsAll,
}

trait TakeAStr {
    fn take_a_str(&self) -> Result<&str>;
}

impl ToString for Value {
    fn to_string(&self) -> String {
        use Value::*;

        match self {
            Letter(v) => v.to_owned(),
            Decimal(v) => v.to_string(),
        }
    }
}

impl TakeAStr for Value {
    fn take_a_str(&self) -> Result<&str> {
        use Value::*;

        match self {
            Letter(v) => Ok(v),
            Decimal(_) => Err(Error::NotAString {
                value: self.clone(),
            }),
        }
    }
}

impl TakeAStr for Vec<Value> {
    fn take_a_str(&self) -> Result<&str> {
        if let Some(first) = self.first() {
            first.take_a_str()
        } else {
            Err(Error::TakeInEmptyList)
        }
    }
}

impl Value {
    pub fn from_str(value_s: &str) -> Self {
        Value::Letter(value_s.to_owned())
    }
}

impl Cont {
    /// 从字符串数据中构建条件。
    pub fn new(
        is_negative: bool,
        field_str: String,
        operator_str: String,
        value: Vec<Value>,
    ) -> Result<Self> {
        let operator =
            Operator::from_str(operator_str.as_str()).map_err(|_| Error::UnknownOperator {
                operator: operator_str.to_owned(),
            })?;

        let field = Field::from_str(field_str.as_str()).map_err(|_| Error::UnknownField {
            field: field_str.to_owned(),
        })?;

        let operators = FIELD_OPERATORS
            .get(field_str.as_str())
            .copied()
            // 没有注册运算符列表，表示字段未启用。
            .ok_or(Error::FieldNotEndabled { field })?;

        // 检查运算符是否支持。
        if !operators.contains(&operator) {
            return Err(Error::UnsupportedOperator { field, operator });
        }

        Ok(Cont {
            is_negative,
            field,
            operator: Some(operator),
            value: Some(value),
        })
    }

    pub fn single_field(is_negative: bool, field_str: String) -> Result<Self> {
        let field = Field::from_str(field_str.as_str()).map_err(|_| Error::UnknownField {
            field: field_str.to_owned(),
        })?;

        let _operators = FIELD_OPERATORS
            .get(field_str.as_str())
            .copied()
            // 没有注册运算符列表，表示字段未启用。
            .ok_or(Error::FieldNotEndabled { field })?;

        Ok(Cont {
            is_negative,
            field,
            operator: None,
            value: None,
        })
    }

    fn operator(&self) -> Result<&Operator> {
        if let Some(operator) = &self.operator {
            Ok(operator)
        } else {
            Err(Error::FieldRequireOperator { field: self.field })
        }
    }

    fn value(&self) -> Result<&Vec<Value>> {
        if let Some(value) = &self.value {
            Ok(value)
        } else {
            Err(Error::FieldRequireValue { field: self.field })
        }
    }
}

impl Matcher {
    pub fn match_message(&mut self, message: &Message) -> Result<bool> {
        self.loop_match(message, 0)
    }

    fn loop_match(&mut self, message: &Message, position: usize) -> Result<bool> {
        if position > 0 && self.is_last_match {
            return Ok(true);
        }
        if position > (self.groups.len() - 1) {
            return Ok(self.is_last_match);
        }

        let conts = unsafe { self.groups.get_unchecked(position) };

        let mut result = true;
        for cont in conts {
            if !cont.match_message(message)? {
                result = false;
                break;
            }
        }
        self.is_last_match = result;
        self.loop_match(message, position + 1)
    }
}

impl Cont {
    pub fn match_message(&self, message: &Message) -> Result<bool> {
        let r = match self.field {
            Field::MessageText => {
                if let Some(text) = message.text.as_ref() {
                    match self.operator()? {
                        Operator::ContainsOne => {
                            let mut result = false;
                            for v in self.value()? {
                                if text.contains(v.take_a_str()?) {
                                    result = true;
                                    break;
                                }
                            }

                            Ok(result)
                        }
                        Operator::ContainsAll => {
                            let mut result = true;
                            for v in self.value()? {
                                if !text.contains(v.take_a_str()?) {
                                    result = false;
                                    break;
                                }
                            }

                            Ok(result)
                        }
                        Operator::Eq => {
                            let result = text.eq(&self.value()?.take_a_str()?);

                            Ok(result)
                        }
                        _ => Err(Error::UnsupportedOperator {
                            field: self.field,
                            operator: *self.operator()?,
                        }),
                    }
                } else {
                    Ok(false)
                }
            }
            Field::MessageTextSize => {
                // TODO：有待实现。
                Ok(false)
            }
            Field::MessageFromIsBot => {
                if let Some(from) = &message.from {
                    Ok(from.is_bot)
                } else {
                    Ok(false)
                }
            }
        };

        if let Ok(no_negative_result) = r {
            if self.is_negative {
                Ok(!no_negative_result)
            } else {
                Ok(no_negative_result)
            }
        } else {
            r
        }
    }
}
