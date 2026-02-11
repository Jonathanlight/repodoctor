# RepoDoctor - Spécification Technique

## User Stories

### Epic 1: Diagnostic

- **US-001**: En tant que développeur, je veux scanner mon projet pour obtenir un diagnostic complet en moins de 30 secondes.
- **US-002**: En tant que développeur, je veux que RepoDoctor détecte automatiquement le langage/framework utilisé.
- **US-003**: En tant que développeur, je veux voir un score de santé global sur 100 avec breakdown par catégorie.

### Epic 2: Analyse

- **US-010**: En tant que développeur, je veux analyser la structure de mon projet contre les conventions du framework.
- **US-011**: En tant que développeur, je veux détecter les dépendances obsolètes ou vulnérables.
- **US-012**: En tant que développeur, je veux identifier les fichiers de config manquants ou incohérents.
- **US-013**: En tant que développeur, je veux évaluer la couverture et la qualité des tests.
- **US-014**: En tant que développeur, je veux détecter les secrets exposés accidentellement.

### Epic 3: Correction

- **US-020**: En tant que développeur, je veux recevoir des suggestions de correction priorisées.
- **US-021**: En tant que développeur, je veux appliquer des corrections automatiques pour les problèmes simples.
- **US-022**: En tant que développeur, je veux un mode dry-run pour prévisualiser les changements.

### Epic 4: Reporting

- **US-030**: En tant que tech lead, je veux générer un rapport HTML/PDF partageable.
- **US-031**: En tant que CTO, je veux un rapport exécutif avec tendances temporelles.
- **US-032**: En tant que développeur, je veux intégrer RepoDoctor dans ma CI/CD.

### Epic 5: Configuration

- **US-040**: En tant qu'équipe, je veux définir des règles custom dans un fichier `.repodoctor.yml`.
- **US-041**: En tant que développeur, je veux ignorer certains fichiers/règles.
- **US-042**: En tant qu'entreprise, je veux des presets de règles (strict, balanced, relaxed).

---

## Architecture Technique

### 1. Stack Technologique

| Composant | Technologie | Justification |
|-----------|-------------|---------------|
| **Langage** | Rust | Performance, binaire unique, cross-platform |
| **CLI Framework** | `clap` v4 | Standard Rust, dérivation proc-macro |
| **Async Runtime** | `tokio` | Scan parallèle des fichiers |
| **Parsing** | `tree-sitter` | AST multi-langages performant |
| **Config** | `serde` + YAML/TOML | Flexibilité, standard industrie |
| **Output** | `colored` + `indicatif` | UX terminal moderne |
| **Reports** | `tera` (templates) | HTML/Markdown génération |
| **Tests** | `cargo test` + `insta` | Snapshot testing pour outputs |

**Alternative Node.js/TypeScript:** Si préférence pour l'écosystème JS, utiliser `commander` + `inquirer` + `ora`.

### 2. Architecture Modulaire

```
repodoctor/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry point CLI
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── scan.rs
│   │   │   ├── fix.rs
│   │   │   ├── report.rs
│   │   │   └── watch.rs
│   │   └── output.rs           # Formatters (json, table, pretty)
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── project.rs          # Project detection & metadata
│   │   ├── scanner.rs          # Orchestrateur de scan
│   │   ├── score.rs            # Calcul du score global
│   │   └── config.rs           # .repodoctor.yml parser
│   │
│   ├── analyzers/              # Un analyzer par domaine
│   │   ├── mod.rs
│   │   ├── traits.rs           # Analyzer trait definition
│   │   ├── structure.rs        # Analyse arborescence
│   │   ├── dependencies.rs     # Deps obsolètes/vulnérables
│   │   ├── config_files.rs     # Cohérence configs
│   │   ├── testing.rs          # Couverture & qualité tests
│   │   ├── security.rs         # Secrets, permissions
│   │   └── documentation.rs    # README, CHANGELOG, etc.
│   │
│   ├── frameworks/             # Règles spécifiques frameworks
│   │   ├── mod.rs
│   │   ├── detector.rs         # Auto-détection framework
│   │   ├── symfony.rs
│   │   ├── flutter.rs
│   │   ├── nextjs.rs
│   │   ├── rust_cargo.rs
│   │   └── generic.rs
│   │
│   ├── fixers/                 # Auto-corrections
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   ├── gitignore.rs
│   │   ├── editorconfig.rs
│   │   └── dependencies.rs
│   │
│   ├── reporters/
│   │   ├── mod.rs
│   │   ├── terminal.rs
│   │   ├── json.rs
│   │   ├── html.rs
│   │   └── markdown.rs
│   │
│   └── utils/
│       ├── mod.rs
│       ├── fs.rs               # File system helpers
│       ├── git.rs              # Git integration
│       └── cache.rs            # Résultats cache
│
├── templates/                  # Tera templates pour reports
│   ├── report.html
│   └── report.md
│
├── presets/                    # Presets de configuration
│   ├── strict.yml
│   ├── balanced.yml
│   └── relaxed.yml
│
└── tests/
    ├── fixtures/               # Projets de test
    │   ├── symfony_healthy/
    │   ├── symfony_sick/
    │   ├── flutter_project/
    │   └── nodejs_messy/
    └── integration/
```

### 3. Interfaces Clés

#### Analyzer Trait

```rust
// src/analyzers/traits.rs

use crate::core::{Project, Issue, Severity};

#[async_trait]
pub trait Analyzer: Send + Sync {
    /// Nom unique de l'analyzer
    fn name(&self) -> &'static str;

    /// Description pour l'aide
    fn description(&self) -> &'static str;

    /// Poids dans le score global (0-100)
    fn weight(&self) -> u8;

    /// Vérifie si cet analyzer s'applique au projet
    fn applies_to(&self, project: &Project) -> bool;

    /// Exécute l'analyse et retourne les issues trouvées
    async fn analyze(&self, project: &Project) -> Result<Vec<Issue>>;
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub id: String,           // "SEC-001"
    pub analyzer: String,     // "security"
    pub severity: Severity,   // Critical, High, Medium, Low, Info
    pub title: String,
    pub description: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub suggestion: Option<String>,
    pub auto_fixable: bool,
    pub references: Vec<String>,  // URLs documentation
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Critical = 100,  // Bloquant
    High = 75,
    Medium = 50,
    Low = 25,
    Info = 0,        // Suggestion
}
```

#### Framework Detector

```rust
// src/frameworks/detector.rs

pub struct FrameworkDetector;

impl FrameworkDetector {
    pub fn detect(path: &Path) -> DetectedProject {
        let indicators = vec![
            // Symfony
            ("symfony.lock", Framework::Symfony),
            ("config/bundles.php", Framework::Symfony),

            // Laravel
            ("artisan", Framework::Laravel),

            // Flutter
            ("pubspec.yaml", Framework::Flutter),

            // Next.js
            ("next.config.js", Framework::NextJs),
            ("next.config.mjs", Framework::NextJs),

            // Rust
            ("Cargo.toml", Framework::RustCargo),

            // Node.js générique
            ("package.json", Framework::NodeJs),

            // Python
            ("pyproject.toml", Framework::Python),
            ("requirements.txt", Framework::Python),
        ];

        // Détection par priorité (plus spécifique d'abord)
        // ...
    }
}

pub struct DetectedProject {
    pub framework: Framework,
    pub language: Language,
    pub version: Option<String>,
    pub package_manager: Option<PackageManager>,
    pub has_git: bool,
    pub has_ci: Option<CIProvider>,
}
```

#### Health Score

```rust
// src/core/score.rs

pub struct HealthScore {
    pub total: u8,              // 0-100
    pub grade: Grade,           // A, B, C, D, F
    pub breakdown: ScoreBreakdown,
}

pub struct ScoreBreakdown {
    pub structure: CategoryScore,    // 20% weight
    pub dependencies: CategoryScore, // 20% weight
    pub configuration: CategoryScore,// 15% weight
    pub testing: CategoryScore,      // 25% weight
    pub security: CategoryScore,     // 15% weight
    pub documentation: CategoryScore,// 5% weight
}

pub struct CategoryScore {
    pub score: u8,
    pub max_score: u8,
    pub issues_count: usize,
    pub critical_count: usize,
}

impl HealthScore {
    pub fn calculate(issues: &[Issue], weights: &Weights) -> Self {
        // Algorithme:
        // 1. Grouper issues par catégorie
        // 2. Calculer pénalités selon sévérité
        // 3. Appliquer les poids
        // 4. Normaliser sur 100
    }

    pub fn grade(&self) -> Grade {
        match self.total {
            90..=100 => Grade::A,
            80..=89 => Grade::B,
            70..=79 => Grade::C,
            60..=69 => Grade::D,
            _ => Grade::F,
        }
    }
}
```

### 4. Configuration (.repodoctor.yml)

```yaml
# Hériter d'un preset
extends: balanced  # strict | balanced | relaxed

# Override global
severity_threshold: medium  # Ignorer les issues < medium

# Exclusions globales
ignore:
  paths:
    - vendor/
    - node_modules/
    - "**/*.generated.*"
  rules:
    - DOC-003  # Pas de CHANGELOG requis

# Configuration par analyzer
analyzers:
  structure:
    enabled: true
    rules:
      max_directory_depth: 6
      forbidden_paths:
        - src/Utils/  # Préférer des noms explicites
      required_paths:
        - src/
        - tests/
        - README.md

  dependencies:
    enabled: true
    rules:
      max_outdated_days: 90
      security_check: true
      license_whitelist:
        - MIT
        - Apache-2.0
        - BSD-3-Clause

  testing:
    enabled: true
    rules:
      min_coverage: 70
      require_unit_tests: true
      require_integration_tests: false
      test_naming_pattern: "*Test.php"  # Symfony

  security:
    enabled: true
    rules:
      check_secrets: true
      secret_patterns:
        - "password\\s*=\\s*['\"][^'\"]+['\"]"
        - "api[_-]?key\\s*=\\s*['\"][^'\"]+['\"]"
      check_permissions: true
      forbidden_permissions:
        - 0777

  documentation:
    enabled: true
    rules:
      require_readme: true
      readme_min_sections:
        - Installation
        - Usage
      require_changelog: false
      require_license: true

# Règles custom
custom_rules:
  - id: CUSTOM-001
    name: "No var_dump in PHP"
    pattern: "var_dump\\s*\\("
    file_pattern: "*.php"
    severity: high
    message: "Remove var_dump() before committing"

# Intégration CI
ci:
  fail_on: critical  # critical | high | medium | any
  badge: true
  trend_tracking: true
```

### 5. Framework-Specific Rules

#### Symfony Analyzer

```rust
pub struct SymfonyAnalyzer;

impl SymfonyAnalyzer {
    fn rules() -> Vec<Rule> {
        vec![
            // Structure
            Rule::new("SYM-001", "Missing src/Controller/ directory", Severity::High),
            Rule::new("SYM-002", "Missing src/Entity/ directory", Severity::Medium),
            Rule::new("SYM-003", "Controllers outside src/Controller/", Severity::Medium),
            Rule::new("SYM-004", "Services not in src/Service/", Severity::Low),

            // Configuration
            Rule::new("SYM-010", "Missing .env.example", Severity::High),
            Rule::new("SYM-011", ".env committed to git", Severity::Critical),
            Rule::new("SYM-012", "APP_SECRET has default value", Severity::Critical),
            Rule::new("SYM-013", "Debug mode in production config", Severity::Critical),
            Rule::new("SYM-014", "Missing doctrine.yaml", Severity::High),
            Rule::new("SYM-015", "Missing security.yaml", Severity::High),

            // Dependencies
            Rule::new("SYM-020", "Symfony version outdated (major)", Severity::High),
            Rule::new("SYM-021", "Dev dependencies in require section", Severity::Medium),
            Rule::new("SYM-022", "Missing symfony/runtime", Severity::Low),

            // Testing
            Rule::new("SYM-030", "Missing phpunit.xml.dist", Severity::Medium),
            Rule::new("SYM-031", "No tests/ directory", Severity::High),
            Rule::new("SYM-032", "PHPUnit not in dev dependencies", Severity::High),

            // Security
            Rule::new("SYM-040", "Hardcoded database credentials", Severity::Critical),
            Rule::new("SYM-041", "Missing CORS configuration", Severity::Medium),
            Rule::new("SYM-042", "Unsafe deserialization detected", Severity::Critical),

            // Best Practices
            Rule::new("SYM-050", "Missing .gitignore entries", Severity::Medium),
            Rule::new("SYM-051", "Composer.lock not committed", Severity::High),
            Rule::new("SYM-052", "Missing rector.php for upgrades", Severity::Info),
            Rule::new("SYM-053", "PHPStan not configured", Severity::Medium),
        ]
    }
}
```

#### Flutter Analyzer

```rust
pub struct FlutterAnalyzer;

impl FlutterAnalyzer {
    fn rules() -> Vec<Rule> {
        vec![
            // Structure
            Rule::new("FLT-001", "Missing lib/ directory", Severity::Critical),
            Rule::new("FLT-002", "Missing test/ directory", Severity::High),
            Rule::new("FLT-003", "Business logic in lib/main.dart", Severity::Medium),
            Rule::new("FLT-004", "No clear architecture (no lib/src/)", Severity::Medium),

            // Configuration
            Rule::new("FLT-010", "pubspec.yaml missing description", Severity::Low),
            Rule::new("FLT-011", "Outdated Flutter SDK constraint", Severity::High),
            Rule::new("FLT-012", "Missing analysis_options.yaml", Severity::High),
            Rule::new("FLT-013", "Lints package not configured", Severity::Medium),

            // Dependencies
            Rule::new("FLT-020", "Outdated dependencies", Severity::Medium),
            Rule::new("FLT-021", "Dev dependency in dependencies", Severity::Medium),
            Rule::new("FLT-022", "Git dependencies in pubspec", Severity::Low),

            // Testing
            Rule::new("FLT-030", "No widget tests", Severity::High),
            Rule::new("FLT-031", "No integration tests", Severity::Medium),
            Rule::new("FLT-032", "Missing flutter_test dependency", Severity::High),

            // Security
            Rule::new("FLT-040", "API keys in source code", Severity::Critical),
            Rule::new("FLT-041", "Insecure HTTP URLs", Severity::High),
            Rule::new("FLT-042", "Debug flags in release", Severity::High),

            // Platform specific
            Rule::new("FLT-050", "Missing Android signing config", Severity::Medium),
            Rule::new("FLT-051", "iOS missing required capabilities", Severity::Medium),
            Rule::new("FLT-052", "Missing app icons", Severity::Low),
        ]
    }
}
```

### 6. CLI Usage

```bash
# Installation
cargo install repodoctor
# ou
npm install -g repodoctor

# Scan basique
repodoctor scan .
repodoctor scan /path/to/project

# Options de scan
repodoctor scan . --format=json          # Output JSON
repodoctor scan . --format=table         # Output table (défaut)
repodoctor scan . --only=security,deps   # Analyzers spécifiques
repodoctor scan . --severity=high        # Filtrer par sévérité min
repodoctor scan . --ci                   # Mode CI (exit code selon issues)

# Score uniquement
repodoctor score .
repodoctor score . --badge               # Génère badge SVG

# Corrections automatiques
repodoctor fix .                         # Mode interactif
repodoctor fix . --auto                  # Tout corriger automatiquement
repodoctor fix . --dry-run               # Prévisualiser sans modifier
repodoctor fix . --only=gitignore        # Fixer un type spécifique

# Rapports
repodoctor report . --html               # Génère report.html
repodoctor report . --md                 # Génère HEALTH.md
repodoctor report . --executive          # Version simplifiée pour management
repodoctor report . --trend              # Inclut historique (si git)

# Configuration
repodoctor init                          # Crée .repodoctor.yml interactif
repodoctor config validate               # Valide la config
repodoctor config show                   # Affiche config effective

# Watch mode
repodoctor watch .                       # Surveille les changements
repodoctor watch . --debounce=5000       # Délai en ms

# Utilitaires
repodoctor version
repodoctor doctor                        # Self-diagnostic
repodoctor frameworks                    # Liste frameworks supportés
```

### 7. Output Exemple

```
$ repodoctor scan .

🩺 RepoDoctor v1.0.0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📁 Project: /home/user/my-symfony-project
🔍 Detected: Symfony 6.4 (PHP 8.2)
⏱️  Scan completed in 2.3s

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🏥 HEALTH SCORE: 72/100 (Grade C)

┌─────────────────┬───────┬────────┬─────────────────────────────┐
│ Category        │ Score │ Issues │ Status                      │
├─────────────────┼───────┼────────┼─────────────────────────────┤
│ 📁 Structure    │ 85/100│   2    │ ✅ Good                     │
│ 📦 Dependencies │ 60/100│   5    │ ⚠️  Needs attention         │
│ ⚙️  Config      │ 70/100│   3    │ ⚠️  Needs attention         │
│ 🧪 Testing      │ 45/100│   4    │ ❌ Poor                     │
│ 🔒 Security     │ 90/100│   1    │ ✅ Good                     │
│ 📚 Docs         │ 80/100│   1    │ ✅ Good                     │
└─────────────────┴───────┴────────┴─────────────────────────────┘

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🚨 CRITICAL (1)

  SEC-011  .env file committed to git
           📍 .env (line 1)
           💡 Add .env to .gitignore and remove from git history
           🔧 Auto-fixable: Yes

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

⚠️  HIGH (4)

  DEP-001  5 outdated dependencies (major versions)
           📍 composer.json
           💡 Run: composer update --with-all-dependencies
           🔧 Auto-fixable: No (requires review)

  TST-001  Test coverage below threshold (42% < 70%)
           📍 phpunit.xml.dist
           💡 Add tests for uncovered classes in src/Service/
           🔧 Auto-fixable: No

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📋 SUMMARY
   • 16 issues found (1 critical, 4 high, 7 medium, 4 low)
   • 6 auto-fixable issues
   • Estimated fix time: ~2 hours

💊 QUICK ACTIONS
   Run: repodoctor fix . --auto    Fix 6 issues automatically
   Run: repodoctor report --html   Generate detailed report
```

---

## Plan de Développement (Sprints)

### Phase 1: MVP (4 semaines)

**Sprint 1 (Semaine 1-2): Core Foundation**
- Setup projet Rust avec Clap
- Détection framework basique (5 frameworks)
- Structure analyzer générique
- Output terminal basique
- Tests unitaires core

**Sprint 2 (Semaine 3-4): Analyzers Essentiels**
- Dependencies analyzer (outdated check)
- Config analyzer (fichiers manquants)
- Security analyzer (secrets basique)
- Score calculation
- Output JSON

### Phase 2: Frameworks (3 semaines)

**Sprint 3: Symfony + PHP**
- Règles Symfony complètes
- Intégration PHPStan results
- Composer.json analysis

**Sprint 4: Flutter + JS**
- Règles Flutter complètes
- Règles Next.js/Node.js
- pubspec.yaml / package.json analysis

### Phase 3: Fixers & Reports (3 semaines)

**Sprint 5: Auto-Fix**
- Fixer trait & framework
- Gitignore fixer
- Editorconfig fixer
- Dry-run mode

**Sprint 6: Reporting**
- HTML report avec charts
- Markdown report
- Badge SVG generation
- Trend tracking (git history)

### Phase 4: Polish & Distribution (2 semaines)

**Sprint 7: UX & Distribution**
- Watch mode
- Init command interactif
- GitHub Action ready
- Homebrew formula
- npm package wrapper
- Documentation complète

---

## Critères de Qualité (DoD)

### Code Quality
- Clippy level max sans erreurs
- Coverage tests > 80%
- Documentation inline complète
- No warnings compilation

### Performance
- Scan < 5s pour projet 10k fichiers
- Memory < 100MB
- Binaire < 10MB

### UX
- Temps réponse premier résultat < 1s
- Messages d'erreur actionnables
- Couleurs accessibles (WCAG AA)
- Mode no-color pour CI

### Distribution
- Binaires Linux/macOS/Windows
- GitHub Releases automatisées
- Changelog automatique

---

## Références & Inspiration

- **SonarQube** — Analyse complète mais lourd/payant
- **ESLint/PHPStan** — Linting ciblé, pas de vision globale
- **Dependabot** — Uniquement dépendances
- **git-secrets** — Uniquement secrets
- **Codeclimate** — SaaS, pas CLI-first
