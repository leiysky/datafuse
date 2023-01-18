// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use common_expression::error_to_null;
use common_expression::types::boolean::BooleanDomain;
use common_expression::types::nullable::NullableDomain;
use common_expression::types::BooleanType;
use common_expression::types::NullableType;
use common_expression::types::StringType;
use common_expression::vectorize_2_arg;
use common_expression::vectorize_with_builder_1_arg;
use common_expression::EvalContext;
use common_expression::FunctionDomain;
use common_expression::FunctionProperty;
use common_expression::FunctionRegistry;
use common_expression::Value;
use common_expression::ValueRef;

pub fn register(registry: &mut FunctionRegistry) {
    registry.register_passthrough_nullable_1_arg::<BooleanType, BooleanType, _, _>(
        "not",
        FunctionProperty::default(),
        |arg| {
            FunctionDomain::Domain(BooleanDomain {
                has_false: arg.has_true,
                has_true: arg.has_false,
            })
        },
        |val, _| match val {
            ValueRef::Scalar(scalar) => Value::Scalar(!scalar),
            ValueRef::Column(column) => Value::Column(!&column),
        },
    );

    // special function to combine the filter efficiently
    registry.register_passthrough_nullable_2_arg::<BooleanType, BooleanType, BooleanType, _, _>(
        "and_filters",
        FunctionProperty::default(),
        |lhs, rhs| {
            FunctionDomain::Domain(BooleanDomain {
                has_false: lhs.has_false || rhs.has_false,
                has_true: lhs.has_true && rhs.has_true,
            })
        },
        |lhs, rhs, _| match (lhs, rhs) {
            (ValueRef::Scalar(true), other) | (other, ValueRef::Scalar(true)) => other.to_owned(),
            (ValueRef::Scalar(false), _) | (_, ValueRef::Scalar(false)) => Value::Scalar(false),
            (ValueRef::Column(a), ValueRef::Column(b)) => Value::Column(&a & &b),
        },
    );

    registry.register_2_arg_core::<BooleanType, BooleanType, BooleanType, _, _>(
        "and",
        FunctionProperty::default(),
        |lhs, rhs| {
            FunctionDomain::Domain(BooleanDomain {
                has_false: lhs.has_false || rhs.has_false,
                has_true: lhs.has_true && rhs.has_true,
            })
        },
        |lhs, rhs, _| match (lhs, rhs) {
            (ValueRef::Scalar(true), other) | (other, ValueRef::Scalar(true)) => other.to_owned(),
            (ValueRef::Scalar(false), _) | (_, ValueRef::Scalar(false)) => Value::Scalar(false),
            (ValueRef::Column(a), ValueRef::Column(b)) => Value::Column(&a & &b),
        },
    );

    registry.register_2_arg_core::<BooleanType, BooleanType, BooleanType, _, _>(
        "or",
        FunctionProperty::default(),
        |lhs, rhs| {
            FunctionDomain::Domain(BooleanDomain {
                has_false: lhs.has_false && rhs.has_false,
                has_true: lhs.has_true || rhs.has_true,
            })
        },
        |lhs, rhs, _| match (lhs, rhs) {
            (ValueRef::Scalar(true), _) | (_, ValueRef::Scalar(true)) => Value::Scalar(true),
            (ValueRef::Scalar(false), other) | (other, ValueRef::Scalar(false)) => other.to_owned(),
            (ValueRef::Column(a), ValueRef::Column(b)) => Value::Column(&a | &b),
        },
    );

    // https://en.wikibooks.org/wiki/Structured_Query_Language/NULLs_and_the_Three_Valued_Logic
    registry.register_2_arg_core::<NullableType<BooleanType>, NullableType<BooleanType>, NullableType<BooleanType>, _, _>(
        "and",
        FunctionProperty::default(),
        |lhs, rhs| {
            if !lhs.has_null && !rhs.has_null {
                let bools = match (&lhs.value, &rhs.value) {
                    (Some(a), Some(b)) => Some(Box::new(BooleanDomain {
                    has_false: a.has_false || b.has_false,
                    has_true: a.has_true && b.has_true,
                    })),
                    _ => return FunctionDomain::Full,
                };
                FunctionDomain::Domain(NullableDomain::<BooleanType> {
                    has_null: false,
                    value: bools,
                })
            } else {
                FunctionDomain::Full
            }
        },
        // value = lhs & rhs,  valid = (lhs_v & rhs_v) | (!lhs & lhs_v) | (!rhs & rhs_v))
        vectorize_2_arg::<NullableType<BooleanType>, NullableType<BooleanType>, NullableType<BooleanType>>(|lhs, rhs, _| {
            match (lhs, rhs) {
                (Some(false), _) => Some(false),
                (_, Some(false))  => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None
             }
        }),
    );

    registry.register_2_arg_core::<NullableType<BooleanType>, NullableType<BooleanType>, NullableType<BooleanType>, _, _>(
        "or",
        FunctionProperty::default(),
        |lhs, rhs| {
            if !lhs.has_null && !rhs.has_null {
                let bools = match (&lhs.value, &rhs.value) {
                    (Some(a), Some(b)) => Some(Box::new(BooleanDomain {
                        has_false: a.has_false && b.has_false,
                        has_true: a.has_true || b.has_true,
                    })),
                    _ => return FunctionDomain::Full,
                };
                FunctionDomain::Domain(NullableDomain::<BooleanType> {
                    has_null: false,
                    value: bools,
                })
            } else {
                FunctionDomain::Full
            }
        },
        // value = lhs | rhs,  valid = (lhs_v & rhs_v) | (lhs_v & lhs) | (rhs_v & rhs)
        vectorize_2_arg::<NullableType<BooleanType>, NullableType<BooleanType>, NullableType<BooleanType>>(|lhs, rhs, _| {
            match (lhs, rhs) {
                (Some(true), _) => Some(true),
                (_, Some(true))  => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None
             }
        }),
    );

    registry.register_passthrough_nullable_2_arg::<BooleanType, BooleanType, BooleanType, _, _>(
        "xor",
        FunctionProperty::default(),
        |lhs, rhs| {
            FunctionDomain::Domain(BooleanDomain {
                has_false: (lhs.has_false && rhs.has_false) || (lhs.has_true && rhs.has_true),
                has_true: (lhs.has_false && rhs.has_true) || (lhs.has_true && rhs.has_false),
            })
        },
        |lhs, rhs, _| match (lhs, rhs) {
            (ValueRef::Scalar(true), ValueRef::Scalar(other))
            | (ValueRef::Scalar(other), ValueRef::Scalar(true)) => Value::Scalar(!other),
            (ValueRef::Scalar(true), ValueRef::Column(other))
            | (ValueRef::Column(other), ValueRef::Scalar(true)) => Value::Column(!&other),
            (ValueRef::Scalar(false), other) | (other, ValueRef::Scalar(false)) => other.to_owned(),
            (ValueRef::Column(a), ValueRef::Column(b)) => {
                Value::Column(common_arrow::arrow::bitmap::xor(&a, &b))
            }
        },
    );

    registry.register_passthrough_nullable_1_arg::<BooleanType, StringType, _, _>(
        "to_string",
        FunctionProperty::default(),
        |_| FunctionDomain::Full,
        eval_boolean_to_string,
    );

    registry.register_combine_nullable_1_arg::<BooleanType, StringType, _, _>(
        "try_to_string",
        FunctionProperty::default(),
        |_| FunctionDomain::Full,
        error_to_null(eval_boolean_to_string),
    );

    registry.register_passthrough_nullable_1_arg::<StringType, BooleanType, _, _>(
        "to_boolean",
        FunctionProperty::default(),
        |_| FunctionDomain::Full,
        eval_string_to_boolean,
    );

    registry.register_combine_nullable_1_arg::<StringType, BooleanType, _, _>(
        "try_to_boolean",
        FunctionProperty::default(),
        |_| FunctionDomain::Full,
        error_to_null(eval_string_to_boolean),
    );
}

fn eval_boolean_to_string(val: ValueRef<BooleanType>, ctx: &mut EvalContext) -> Value<StringType> {
    vectorize_with_builder_1_arg::<BooleanType, StringType>(|val, output, _| {
        output.put_str(if val { "true" } else { "false" });
        output.commit_row();
    })(val, ctx)
}

fn eval_string_to_boolean(val: ValueRef<StringType>, ctx: &mut EvalContext) -> Value<BooleanType> {
    vectorize_with_builder_1_arg::<StringType, BooleanType>(|val, output, ctx| {
        if val.eq_ignore_ascii_case(b"true") {
            output.push(true);
        } else if val.eq_ignore_ascii_case(b"false") {
            output.push(false);
        } else {
            ctx.set_error(output.len(), "unable to parse string to type `BOOLEAN`");
            output.push(false);
        }
    })(val, ctx)
}