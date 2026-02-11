use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::analyzers::traits::{AnalyzerCategory, Issue, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    A,
    B,
    C,
    D,
    F,
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grade::A => write!(f, "A"),
            Grade::B => write!(f, "B"),
            Grade::C => write!(f, "C"),
            Grade::D => write!(f, "D"),
            Grade::F => write!(f, "F"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub name: String,
    pub score: u8,
    pub issues_count: usize,
    pub critical_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub total: u8,
    pub grade: Grade,
    pub breakdown: Vec<CategoryScore>,
}

impl HealthScore {
    pub fn calculate(issues: &[Issue]) -> Self {
        let weights: HashMap<AnalyzerCategory, f64> = HashMap::from([
            (AnalyzerCategory::Structure, 0.20),
            (AnalyzerCategory::Dependencies, 0.20),
            (AnalyzerCategory::Configuration, 0.15),
            (AnalyzerCategory::Testing, 0.25),
            (AnalyzerCategory::Security, 0.15),
            (AnalyzerCategory::Documentation, 0.05),
        ]);

        let mut category_issues: HashMap<AnalyzerCategory, Vec<&Issue>> = HashMap::new();
        for issue in issues {
            category_issues
                .entry(issue.category.clone())
                .or_default()
                .push(issue);
        }

        let mut breakdown = Vec::new();
        let mut weighted_total: f64 = 0.0;
        let mut total_weight: f64 = 0.0;

        let categories = [
            AnalyzerCategory::Structure,
            AnalyzerCategory::Dependencies,
            AnalyzerCategory::Configuration,
            AnalyzerCategory::Testing,
            AnalyzerCategory::Security,
            AnalyzerCategory::Documentation,
        ];

        for category in &categories {
            let weight = weights.get(category).copied().unwrap_or(0.0);
            let cat_issues = category_issues.get(category);

            let mut score: i32 = 100;
            let mut issues_count = 0;
            let mut critical_count = 0;

            if let Some(issues) = cat_issues {
                issues_count = issues.len();
                for issue in issues {
                    score -= issue.severity.penalty() as i32;
                    if issue.severity == Severity::Critical {
                        critical_count += 1;
                    }
                }
            }

            let clamped_score = score.clamp(0, 100) as u8;

            breakdown.push(CategoryScore {
                name: category.to_string(),
                score: clamped_score,
                issues_count,
                critical_count,
            });

            weighted_total += clamped_score as f64 * weight;
            total_weight += weight;
        }

        let total = if total_weight > 0.0 {
            (weighted_total / total_weight).round() as u8
        } else {
            100
        };

        let grade = match total {
            90..=100 => Grade::A,
            80..=89 => Grade::B,
            70..=79 => Grade::C,
            60..=69 => Grade::D,
            _ => Grade::F,
        };

        HealthScore {
            total,
            grade,
            breakdown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_issue(category: AnalyzerCategory, severity: Severity) -> Issue {
        Issue {
            id: "TST-001".to_string(),
            analyzer: "test".to_string(),
            category,
            severity,
            title: "Test".to_string(),
            description: "Test issue".to_string(),
            file: None,
            line: None,
            suggestion: None,
            auto_fixable: false,
            references: vec![],
        }
    }

    #[test]
    fn test_perfect_score_no_issues() {
        let score = HealthScore::calculate(&[]);
        assert_eq!(score.total, 100);
        assert_eq!(score.grade, Grade::A);
    }

    #[test]
    fn test_grade_a() {
        let score = HealthScore::calculate(&[
            make_issue(AnalyzerCategory::Documentation, Severity::Low),
        ]);
        assert!(score.total >= 90);
        assert_eq!(score.grade, Grade::A);
    }

    #[test]
    fn test_critical_issue_lowers_score() {
        let score = HealthScore::calculate(&[
            make_issue(AnalyzerCategory::Security, Severity::Critical),
        ]);
        assert!(score.total < 100);
    }

    #[test]
    fn test_multiple_issues_compound() {
        let issues = vec![
            make_issue(AnalyzerCategory::Structure, Severity::High),
            make_issue(AnalyzerCategory::Structure, Severity::High),
            make_issue(AnalyzerCategory::Structure, Severity::Medium),
        ];
        let score = HealthScore::calculate(&issues);
        let structure_score = score.breakdown.iter().find(|b| b.name == "Structure").unwrap();
        // 100 - 15 - 15 - 8 = 62
        assert_eq!(structure_score.score, 62);
    }

    #[test]
    fn test_score_clamped_at_zero() {
        let issues: Vec<Issue> = (0..10)
            .map(|_| make_issue(AnalyzerCategory::Security, Severity::Critical))
            .collect();
        let score = HealthScore::calculate(&issues);
        let security_score = score.breakdown.iter().find(|b| b.name == "Security").unwrap();
        assert_eq!(security_score.score, 0);
    }

    #[test]
    fn test_grade_f() {
        let mut issues = Vec::new();
        for cat in [
            AnalyzerCategory::Structure,
            AnalyzerCategory::Dependencies,
            AnalyzerCategory::Configuration,
            AnalyzerCategory::Testing,
            AnalyzerCategory::Security,
        ] {
            for _ in 0..5 {
                issues.push(make_issue(cat.clone(), Severity::Critical));
            }
        }
        let score = HealthScore::calculate(&issues);
        assert_eq!(score.grade, Grade::F);
    }

    #[test]
    fn test_breakdown_has_all_categories() {
        let score = HealthScore::calculate(&[]);
        assert_eq!(score.breakdown.len(), 6);
    }

    #[test]
    fn test_category_score_counts() {
        let issues = vec![
            make_issue(AnalyzerCategory::Testing, Severity::Critical),
            make_issue(AnalyzerCategory::Testing, Severity::High),
            make_issue(AnalyzerCategory::Testing, Severity::Low),
        ];
        let score = HealthScore::calculate(&issues);
        let testing = score.breakdown.iter().find(|b| b.name == "Testing").unwrap();
        assert_eq!(testing.issues_count, 3);
        assert_eq!(testing.critical_count, 1);
    }
}
