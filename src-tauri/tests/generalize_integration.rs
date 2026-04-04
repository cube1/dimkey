mod common;

use dimkey_lib::desensitizer::generalize::apply_generalize;
use dimkey_lib::models::sensitive::SensitiveType;

/// S07: 地址泛化验证 — 省市区层级
#[test]
fn test_generalize_address_levels() {
    // 详细地址应泛化掉门牌号
    let result = apply_generalize("北京市朝阳区建国路88号", &SensitiveType::Address);
    assert!(result.contains("北京"), "应保留'北京': {}", result);
    assert!(!result.contains("88号"), "不应保留门牌号'88号': {}", result);

    let result2 = apply_generalize("上海市浦东新区陆家嘴环路1000号", &SensitiveType::Address);
    assert!(result2.contains("上海"), "应保留'上海': {}", result2);
    assert!(!result2.contains("1000号"), "不应保留门牌号: {}", result2);

    let result3 = apply_generalize("深圳市南山区科技园南路18号", &SensitiveType::Address);
    assert!(result3.contains("深圳"), "应保留'深圳': {}", result3);
}

/// S08: 身份证泛化验证 — 提取出生年份
#[test]
fn test_generalize_idcard() {
    let result = apply_generalize("110101199003076789", &SensitiveType::IdCard);
    // 泛化后应提取出生年份（身份证第7-10位）
    assert!(result.contains("1990"), "应包含出生年份'1990': {}", result);
    assert!(result.contains("年出生"), "应包含'年出生': {}", result);
    // 不应包含完整原始信息
    assert_ne!(result, "110101199003076789", "不应与原文相同");
    assert!(!result.contains("110101"), "不应保留地区码: {}", result);

    let result2 = apply_generalize("320106198805151235", &SensitiveType::IdCard);
    assert!(result2.contains("1988"), "应包含出生年份'1988': {}", result2);
    assert!(result2.contains("年出生"), "应包含'年出生': {}", result2);
    assert_ne!(result2, "320106198805151235");
}
