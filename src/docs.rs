use std::collections::HashMap;

pub struct DocEntry {
    pub signature: &'static str,
    pub doc: &'static str,
    pub params: &'static [&'static str],
    pub returns: &'static str,
    pub examples: &'static [&'static str],
    pub category: &'static str,
}

impl DocEntry {
    pub fn new(sig: &'static str, doc: &'static str, params: &'static [&'static str], returns: &'static str, examples: &'static [&'static str], cat: &'static str) -> Self {
        DocEntry { signature: sig, doc, params, returns, examples, category: cat }
    }

    /// Generate a snippet string for completion: `name(${1:param1}, ${2:param2})`
    pub fn snippet(&self, name: &str) -> String {
        // Take the first signature variant (before `  |  `)
        let sig = self.signature.split("  |  ").next().unwrap_or(self.signature);
        // Extract the part between parentheses
        let paren_start = sig.find('(');
        let paren_end = sig.rfind(')');
        let params_str = match (paren_start, paren_end) {
            (Some(s), Some(e)) if s < e => &sig[s + 1..e],
            _ => "",
        };

        // Split by comma, extract parameter names
        let param_names: Vec<&str> = params_str.split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.'))
            .collect();

        if param_names.is_empty() {
            return format!("{}()", name);
        }

        let mut result = format!("{}(", name);
        for (i, p) in param_names.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&format!("${{{}:{}}}", i + 1, p));
        }
        result.push(')');
        result
    }
}

/// Parse a signature string into name and parameter list for snippet generation
pub fn parse_signature_for_snippet(sig: &str, name: &str) -> String {
    let sig = sig.split("  |  ").next().unwrap_or(sig);
    let paren_start = sig.find('(');
    let paren_end = sig.rfind(')');
    let params_str = match (paren_start, paren_end) {
        (Some(s), Some(e)) if s < e => &sig[s + 1..e],
        _ => "",
    };

    let param_names: Vec<&str> = params_str.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();

    if param_names.is_empty() {
        return format!("{}()", name);
    }
    let mut out = format!("{}(", name);
    for (i, pn) in param_names.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push_str(&format!("${{{}:{}}}", i + 1, pn));
    }
    out.push(')');
    out
}

macro_rules! doc {
    ($sig:expr, $doc:expr, $params:expr, $returns:expr, $examples:expr, $cat:expr) => {
        DocEntry::new($sig, $doc, $params, $returns, $examples, $cat)
    };
}

pub struct Builtins {
    pub functions: HashMap<&'static str, DocEntry>,
    pub constants: HashMap<&'static str, DocEntry>,
    pub keywords: Vec<&'static str>,
    pub operators: Vec<&'static str>,
}

impl Builtins {
    pub fn new() -> Self {
        let mut f: HashMap<&'static str, DocEntry> = HashMap::new();

        // ═══════════════════════════════════════════════════
        // ALGEBRA
        // ═══════════════════════════════════════════════════
        f.insert("expand", doc!("expand(expr)", "Expands an expression by distributing products over sums and applying other algebraic identities.",
            &["expr - expression to expand"], "Expanded expression",
            &["expand((x + y)^3);  → x^3 + 3 x^2 y + 3 x y^2 + y^3", "expand((x + 1)*(x - 1));  → x^2 - 1"], "algebra"));

        f.insert("factor", doc!("factor(expr)", "Factors an expression into irreducible factors over the integers.",
            &["expr - expression to factor"], "Factored expression",
            &["factor(x^2 - 1);  → (x - 1) (x + 1)", "factor(x^3 + y^3);  → (x + y) (x^2 - x y + y^2)"], "algebra"));

        f.insert("gfactor", doc!("gfactor(expr)", "Factors an expression over the Gaussian integers (includes %i).",
            &["expr - expression to factor"], "Factored expression over Gaussian integers",
            &["gfactor(x^2 + 1);  → (x - %i) (x + %i)"], "algebra"));

        f.insert("ratsimp", doc!("ratsimp(expr)", "Simplifies a rational expression (ratio of polynomials) to a canonical form.",
            &["expr - rational expression to simplify"], "Simplified rational expression",
            &["ratsimp((x^2 - 1)/(x + 1));  → x - 1", "ratsimp(1/x + 1/(x+1));  → (2 x + 1)/(x^2 + x)"], "algebra"));

        f.insert("fullratsimp", doc!("fullratsimp(expr)", "Repeatedly applies ratsimp and non-rational simplification until the expression stabilizes.",
            &["expr - expression to simplify"], "Fully simplified expression",
            &["fullratsimp((x^(a/2) - 1)*(x^(a/2) + 1)/(x^a - 1));  → 1"], "algebra"));

        f.insert("radcan", doc!("radcan(expr)", "Simplifies expressions containing radicals, logs, and exponents. Unlike ratsimp, radcan handles nested radicals and rational powers.",
            &["expr - expression with radicals/logs/exponents"], "Simplified expression",
            &["radcan(sqrt(x^2 + 2*x + 1));  → x + 1", "radcan(log(exp(x)));  → x"], "algebra"));

        f.insert("logcontract", doc!("logcontract(expr)", "Contracts logarithms using log rules: log(a) + log(b) → log(a*b), n*log(a) → log(a^n).",
            &["expr - expression with log terms"], "Expression with contracted logs",
            &["logcontract(log(x) + log(y));  → log(x*y)", "logcontract(2*log(x) - log(y));  → log(x^2/y)"], "algebra"));

        f.insert("rootscontract", doc!("rootscontract(expr)", "Contracts roots using the rule sqrt(a)*sqrt(b) → sqrt(a*b).",
            &["expr - expression with root terms"], "Expression with contracted roots",
            &["rootscontract(sqrt(x)*sqrt(y));  → sqrt(x*y)"], "algebra"));

        f.insert("partfrac", doc!("partfrac(expr, var)", "Decomposes a rational expression into partial fractions with respect to var.",
            &["expr - rational expression", "var - variable for decomposition"], "Partial fraction decomposition",
            &["partfrac(1/(x^2 - 1), x);  → 1/(2*(x - 1)) - 1/(2*(x + 1))"], "algebra"));

        f.insert("subst", doc!("subst(x, y, expr)", "Substitutes x for y in expr. Performs syntactic substitution.",
            &["x - replacement expression", "y - expression to replace", "expr - target expression"], "Expression with substitution applied",
            &["subst(5, x, x^2 + x + 1);  → 31", "subst(cos(x), sin(x), sin(x)^2);  → cos(x)^2"], "algebra"));

        f.insert("psubst", doc!("psubst(list, expr)", "Parallel substitution. Substitutes multiple values simultaneously (no cascading).",
            &["list - [[eq1, val1], [eq2, val2], ...] or [eq1=val1, eq2=val2, ...]", "expr - target expression"], "Expression with substitutions applied in parallel",
            &["psubst([x=1, y=2], x + y);  → 3"], "algebra"));

        f.insert("ratsubst", doc!("ratsubst(a, b, expr)", "Rational substitution: substitutes a for b in expr, simplifying rationally.",
            &["a - replacement", "b - pattern to replace", "expr - target expression"], "Expression with rational substitution applied",
            &["ratsubst(sin(x), x^2, x^2 + x^3);  → sin(x) + x*sin(x)"], "algebra"));

        f.insert("coeff", doc!("coeff(expr, var, n)", "Extracts the coefficient of var^n in expr.",
            &["expr - polynomial expression", "var - variable", "n - power (default 1)"], "Coefficient of var^n",
            &["coeff(2*x^2 + 3*x + 5, x, 2);  → 2", "coeff(2*x^2 + 3*x + 5, x, 0);  → 5"], "algebra"));

        f.insert("ratcoef", doc!("ratcoef(expr, var, n)", "Extracts the rational coefficient of var^n in expr. Similar to coeff but works on CRE form.",
            &["expr - rational expression", "var - variable", "n - power (default 1)"], "Rational coefficient",
            &["ratcoef(2*x^2 + 3*x + 5, x, 2);  → 2"], "algebra"));

        f.insert("bothcoef", doc!("bothcoef(expr, var)", "Returns the coefficient of var in expr and the remaining expression.",
            &["expr - expression", "var - variable"], "[coefficient, remainder]",
            &["bothcoef(2*x + 3, x);  → [2, 3]"], "algebra"));

        f.insert("collectterms", doc!("collectterms(expr, var1, var2, ...)", "Collects coefficients of the specified variables, grouping terms.",
            &["expr - expression", "vars - variables to collect"], "Expression with collected terms",
            &["collectterms((x + y)*(x + 1), x);  → x^2 + (y + 1)*x + y"], "algebra"));

        f.insert("isolate", doc!("isolate(expr, var)", "Isolates terms containing var in expr, returning an expression of the form var = expression.",
            &["expr - equation or expression", "var - variable to isolate"], "Isolated expression",
            &["isolate(x^2 + 2*x + 1 = 0, x);  → x = [-1, -1]"], "algebra"));

        f.insert("eliminate", doc!("eliminate([eqns], [vars])", "Eliminates variables from a set of equations, returning a single expression.",
            &["eqns - list of equations", "vars - variables to eliminate"], "Expression with variables eliminated",
            &["eliminate([x + y = 5, x - y = 1], [y]);  → [2*x - 6]"], "algebra"));

        f.insert("resultant", doc!("resultant(expr1, expr2, var)", "Computes the resultant of two polynomials, eliminating var.",
            &["expr1, expr2 - polynomials", "var - variable to eliminate"], "Resultant", &[], "algebra"));

        f.insert("divide", doc!("divide(poly1, poly2, var)", "Divides poly1 by poly2, returning [quotient, remainder].",
            &["poly1 - dividend polynomial", "poly2 - divisor polynomial", "var - variable"], "[quotient, remainder]",
            &["divide(x^2 + 3*x + 2, x + 1, x);  → [x + 2, 0]"], "algebra"));

        f.insert("quotient", doc!("quotient(poly1, poly2, var)", "Returns the quotient of poly1 divided by poly2.",
            &["poly1, poly2 - polynomials", "var - variable"], "Quotient",
            &["quotient(x^2 + 3*x + 2, x + 1, x);  → x + 2"], "algebra"));

        f.insert("remainder", doc!("remainder(poly1, poly2, var)", "Returns the remainder of poly1 divided by poly2.",
            &["poly1, poly2 - polynomials", "var - variable"], "Remainder",
            &["remainder(x^2 + 3*x + 2, x + 1, x);  → 0"], "algebra"));

        f.insert("solve", doc!("solve(expr, var)  |  solve([eqns], [vars])",
            "Solves equation(s) for variable(s). If expr is not an equation, it solves expr = 0.",
            &["expr - equation or expression (= 0 assumed)", "var - variable(s) to solve for"],
            "List of solutions",
            &["solve(x^2 - 4 = 0, x);  → [x = -2, x = 2]", "solve([x + y = 5, x - y = 1], [x, y]);  → [[x = 3, y = 2]]",
              "solve(x^2 + 1 = 0, x);  → [x = -%i, x = %i]"], "algebra"));

        f.insert("linsolve", doc!("linsolve([eqns], [vars])",
            "Solves a system of linear equations.",
            &["eqns - list of linear equations", "vars - list of variables to solve for"],
            "List of solutions",
            &["linsolve([x + y = 5, 2*x - y = 1], [x, y]);  → [x = 2, y = 3]"], "algebra"));

        f.insert("allroots", doc!("allroots(expr)  |  allroots(eqn)",
            "Finds all real and complex roots of a polynomial. Unlike solve, allroots uses numerical methods.",
            &["expr - polynomial  (or equation)"], "List of numerical roots",
            &["allroots(x^3 + x^2 - x - 1);  → [x = -1.0, x = -1.0, x = 1.0]"], "algebra"));

        f.insert("realroots", doc!("realroots(expr, tol)",
            "Finds real roots of a polynomial within tolerance tol (default 1e-7).",
            &["expr - polynomial (or equation)", "tol - tolerance (optional)"], "List of real roots",
            &["realroots(x^3 - 2, 1e-6);  → [x = 1250000/1048577]"], "algebra"));

        f.insert("gcd", doc!("gcd(a, b)", "Returns the greatest common divisor of a and b.",
            &["a, b - integers or polynomials"], "Greatest common divisor",
            &["gcd(12, 18);  → 6", "gcd(x^2 - 1, x^2 + 2*x + 1);  → x + 1"], "algebra"));

        f.insert("lcm", doc!("lcm(a, b)", "Returns the least common multiple of a and b.",
            &["a, b - integers or polynomials"], "Least common multiple",
            &["lcm(12, 18);  → 36"], "algebra"));

        f.insert("polymod", doc!("polymod(poly, modulus)", "Reduces a polynomial modulo an integer modulus.",
            &["poly - polynomial", "modulus - integer modulus"], "Reduced polynomial",
            &["polymod(x^2 + 3*x + 2, 3);  → x^2 + 2"], "algebra"));

        // ═══════════════════════════════════════════════════
        // CALCULUS
        // ═══════════════════════════════════════════════════
        f.insert("diff", doc!("diff(expr, var1, n1, var2, n2, ...)",
            "Differentiates expr with respect to each var ni times. diff(expr, var) computes the first derivative.",
            &["expr - expression to differentiate", "var - variable(s) to differentiate with respect to", "n - order(s) of differentiation (optional, default 1)"],
            "Derivative of expr",
            &["diff(sin(x), x);  → cos(x)", "diff(x^2*y, x, 1, y, 1);  → 2*x",
              "diff(sin(x), x, 2);  → -sin(x)"], "calculus"));

        f.insert("integrate", doc!("integrate(expr, var)  |  integrate(expr, var, lo, hi)",
            "Integrates expr with respect to var. With limits lo and hi, computes definite integral.",
            &["expr - expression to integrate", "var - integration variable", "lo - lower limit (optional)", "hi - upper limit (optional)"],
            "Integral (indefinite or definite)",
            &["integrate(sin(x), x);  → -cos(x)", "integrate(x^2, x, 0, 1);  → 1/3",
              "integrate(1/(1 + x^2), x);  → atan(x)"], "calculus"));

        f.insert("sum", doc!("sum(expr, var, lo, hi)",
            "Computes the sum of expr as var goes from lo to hi.",
            &["expr - expression to sum", "var - summation index", "lo - lower bound", "hi - upper bound"],
            "Summation result",
            &["sum(k^2, k, 1, n);  → (2*n^3 + 3*n^2 + n)/6", "sum(x^k/k!, k, 0, inf);  → exp(x)"], "calculus"));

        f.insert("product", doc!("product(expr, var, lo, hi)",
            "Computes the product of expr as var goes from lo to hi.",
            &["expr - expression to multiply", "var - product index", "lo - lower bound", "hi - upper bound"],
            "Product result",
            &["product(k, k, 1, 5);  → 120", "product(x + k, k, 0, n);  → x*(x+1)*...*(x+n)"], "calculus"));

        f.insert("limit", doc!("limit(expr, var, val, dir)",
            "Computes the limit of expr as var approaches val from direction dir.",
            &["expr - expression", "var - variable", "val - limit point", "dir - direction: plus (from above), minus (from below) (optional)"],
            "Limit value",
            &["limit(sin(x)/x, x, 0);  → 1", "limit(1/x, x, 0, plus);  → inf", "limit(1/x, x, 0, minus);  → minf"], "calculus"));

        f.insert("taylor", doc!("taylor(expr, var, pt, n)",
            "Computes the Taylor series expansion of expr about pt up to order n.",
            &["expr - expression to expand", "var - variable", "pt - expansion point", "n - order of expansion"],
            "Taylor series",
            &["taylor(sin(x), x, 0, 5);  → x - x^3/6 + x^5/120", "taylor(exp(x), x, 0, 3);  → 1 + x + x^2/2 + x^3/6"], "calculus"));

        f.insert("series", doc!("series(expr, var, pt, n, ...)",
            "General series expansion of expr about pt.",
            &["expr - expression", "var - variable", "pt - expansion point", "n - order"],
            "Series expansion", &[], "calculus"));

        f.insert("laplace", doc!("laplace(expr, t, s)",
            "Computes the Laplace transform of expr with respect to t, giving a function of s.",
            &["expr - expression in t", "t - time variable", "s - Laplace variable"],
            "Laplace transform F(s)",
            &["laplace(exp(a*t), t, s);  → 1/(s - a)", "laplace(sin(a*t), t, s);  → a/(s^2 + a^2)"], "calculus"));

        f.insert("ilt", doc!("ilt(expr, s, t)",
            "Computes the inverse Laplace transform of expr with respect to s, giving a function of t.",
            &["expr - expression in s (Laplace domain)", "s - Laplace variable", "t - time variable"],
            "Inverse Laplace transform f(t)",
            &["ilt(1/(s^2 + 1), s, t);  → sin(t)"], "calculus"));

        f.insert("residue", doc!("residue(expr, var, pt)",
            "Computes the residue of expr at the point pt with respect to var.",
            &["expr - meromorphic expression", "var - variable", "pt - point"], "Residue",
            &["residue(sin(x)/x^2, x, 0);  → 1"], "calculus"));

        f.insert("ode2", doc!("ode2(ode, depvar, indvar)",
            "Solves ordinary differential equations of 1st and 2nd order.",
            &["ode - differential equation", "depvar - dependent variable", "indvar - independent variable"],
            "General solution or false",
            &["ode2('diff(y, x) + y, y, x);  → y = %c*exp(-x)"], "calculus"));

        f.insert("ic1", doc!("ic1(solution, xval, yval)",
            "Applies initial condition to solution from ode2 (1st order).",
            &["solution - general solution from ode2", "xval - x = value", "yval - y = value"], "Particular solution",
            &["ic1(y = %c*exp(-x), x = 0, y = 1);  → y = exp(-x)"], "calculus"));

        f.insert("changevar", doc!("changevar(expr, f(x), g(y), x, y)",
            "Performs change of variable in an integral expression.",
            &["expr - expression containing integral", "f(x) - new variable equation", "g(y) - old variable", "x - new variable", "y - old variable"],
            "Expression with changed variable", &[], "calculus"));

        f.insert("at", doc!("at(expr, [eqn1, eqn2, ...])",
            "Evaluates expr with specified variable values substituted.",
            &["expr - expression", "eqns - equations specifying values"], "Evaluated expression",
            &["at(x^2 + y^2, [x = 1, y = 2]);  → 5"], "calculus"));

        f.insert("atvalue", doc!("atvalue(expr, [var = val], value)",
            "Assigns a value to expr at a specific point for use in Laplace transforms.",
            &["expr - expression", "var = val - point specification", "value - value at that point"],
            "Assigned value", &[], "calculus"));

        f.insert("gradef", doc!("gradef(f(x), g(x))",
            "Defines the gradient/derivative of a function f as g.",
            &["f(x) - function", "g(x) - its derivative"], "Derivative definition", &[], "calculus"));

        f.insert("depends", doc!("depends([f1, f2, ...], [x1, x2, ...])",
            "Declares functional dependencies for differentiation.",
            &["functions - function names", "variables - variables they depend on"], "List of dependencies",
            &["depends([f], [x, y]); diff(f, x);  → 'diff(f, x)"], "calculus"));

        // ═══════════════════════════════════════════════════
        // TRIGONOMETRY
        // ═══════════════════════════════════════════════════
        f.insert("sin", doc!("sin(x)", "Sine of x (x in radians).",
            &["x - angle in radians"], "Sine value",
            &["sin(%pi/2);  → 1", "sin(0);  → 0"], "trigonometry"));
        f.insert("cos", doc!("cos(x)", "Cosine of x (x in radians).",
            &["x - angle in radians"], "Cosine value", &["cos(%pi);  → -1", "cos(0);  → 1"], "trigonometry"));
        f.insert("tan", doc!("tan(x)", "Tangent of x (x in radians).",
            &["x - angle in radians"], "Tangent value", &["tan(%pi/4);  → 1"], "trigonometry"));
        f.insert("cot", doc!("cot(x)", "Cotangent of x (x in radians).", &["x - angle in radians"], "Cotangent value", &[], "trigonometry"));
        f.insert("sec", doc!("sec(x)", "Secant of x: 1/cos(x).", &["x - angle in radians"], "Secant value", &[], "trigonometry"));
        f.insert("csc", doc!("csc(x)", "Cosecant of x: 1/sin(x).", &["x - angle in radians"], "Cosecant value", &[], "trigonometry"));

        f.insert("asin", doc!("asin(x)", "Arc sine (inverse sine) of x. Returns angle in radians in [-%pi/2, %pi/2].",
            &["x - value between -1 and 1"], "Angle in radians", &["asin(1);  → %pi/2"], "trigonometry"));
        f.insert("acos", doc!("acos(x)", "Arc cosine (inverse cosine) of x. Returns angle in radians in [0, %pi].",
            &["x - value between -1 and 1"], "Angle in radians", &["acos(0);  → %pi/2"], "trigonometry"));
        f.insert("atan", doc!("atan(x)", "Arc tangent (inverse tangent) of x. Returns angle in radians in (-%pi/2, %pi/2).",
            &["x - real number"], "Angle in radians", &["atan(1);  → %pi/4"], "trigonometry"));
        f.insert("acot", doc!("acot(x)", "Arc cotangent of x. Returns angle in radians.", &["x - real number"], "Angle in radians", &[], "trigonometry"));
        f.insert("asec", doc!("asec(x)", "Arc secant of x.", &["x - real number"], "Angle in radians", &[], "trigonometry"));
        f.insert("acsc", doc!("acsc(x)", "Arc cosecant of x.", &["x - real number"], "Angle in radians", &[], "trigonometry"));

        f.insert("sinh", doc!("sinh(x)", "Hyperbolic sine of x.", &["x - real number"], "Hyperbolic sine", &[], "trigonometry"));
        f.insert("cosh", doc!("cosh(x)", "Hyperbolic cosine of x.", &["x - real number"], "Hyperbolic cosine", &[], "trigonometry"));
        f.insert("tanh", doc!("tanh(x)", "Hyperbolic tangent of x.", &["x - real number"], "Hyperbolic tangent", &[], "trigonometry"));
        f.insert("coth", doc!("coth(x)", "Hyperbolic cotangent of x.", &["x - real number"], "Hyperbolic cotangent", &[], "trigonometry"));
        f.insert("sech", doc!("sech(x)", "Hyperbolic secant of x.", &["x - real number"], "Hyperbolic secant", &[], "trigonometry"));
        f.insert("csch", doc!("csch(x)", "Hyperbolic cosecant of x.", &["x - real number"], "Hyperbolic cosecant", &[], "trigonometry"));

        f.insert("asinh", doc!("asinh(x)", "Inverse hyperbolic sine of x.", &["x - real number"], "Inverse hyperbolic sine", &[], "trigonometry"));
        f.insert("acosh", doc!("acosh(x)", "Inverse hyperbolic cosine of x.", &["x - real number (>= 1)"], "Inverse hyperbolic cosine", &[], "trigonometry"));
        f.insert("atanh", doc!("atanh(x)", "Inverse hyperbolic tangent of x.", &["x - real number (|x| < 1)"], "Inverse hyperbolic tangent", &[], "trigonometry"));
        f.insert("acoth", doc!("acoth(x)", "Inverse hyperbolic cotangent of x.", &["x - real number (|x| > 1)"], "Inverse hyperbolic cotangent", &[], "trigonometry"));
        f.insert("asech", doc!("asech(x)", "Inverse hyperbolic secant of x.", &["x - real number (0 < x <= 1)"], "Inverse hyperbolic secant", &[], "trigonometry"));
        f.insert("acsch", doc!("acsch(x)", "Inverse hyperbolic cosecant of x.", &["x - non-zero real number"], "Inverse hyperbolic cosecant", &[], "trigonometry"));

        f.insert("trigexpand", doc!("trigexpand(expr)", "Expands trigonometric functions of sums of angles.",
            &["expr - trigonometric expression"], "Expanded expression",
            &["trigexpand(sin(x + y));  → cos(y) sin(x) + sin(y) cos(x)"], "trigonometry"));

        f.insert("trigreduce", doc!("trigreduce(expr)", "Reduces powers and products of trig functions into linear terms.",
            &["expr - trigonometric expression"], "Reduced expression",
            &["trigreduce(sin(x)^2);  → (1 - cos(2*x))/2"], "trigonometry"));

        f.insert("trigsimp", doc!("trigsimp(expr)", "Simplifies trigonometric expressions using identities.",
            &["expr - trigonometric expression"], "Simplified expression",
            &["trigsimp(sin(x)^2 + cos(x)^2);  → 1"], "trigonometry"));

        f.insert("trigrat", doc!("trigrat(expr)", "Converts trigonometric expression to a rational form in sin and cos.",
            &["expr - trigonometric expression"], "Rationalized expression", &[], "trigonometry"));

        // ═══════════════════════════════════════════════════
        // MATRIX / LINEAR ALGEBRA
        // ═══════════════════════════════════════════════════
        f.insert("matrix", doc!("matrix(row1, row2, ...)",
            "Creates a matrix from the given row lists.",
            &["row1, row2, ... - each row is a list [a1, a2, ...]"], "Matrix",
            &["matrix([1, 2], [3, 4]);  → matrix([1, 2], [3, 4])"], "matrix"));

        f.insert("determinant", doc!("determinant(mat)",
            "Computes the determinant of a square matrix.",
            &["mat - square matrix"], "Determinant",
            &["determinant(matrix([a, b], [c, d]));  → a*d - b*c"], "matrix"));

        f.insert("invert", doc!("invert(mat)",
            "Computes the inverse of a square matrix.",
            &["mat - square matrix"], "Inverse matrix",
            &["invert(matrix([1, 2], [3, 4]));  → matrix([-2, 1], [3/2, -1/2])"], "matrix"));

        f.insert("transpose", doc!("transpose(mat)",
            "Transposes a matrix (swap rows and columns).",
            &["mat - matrix"], "Transposed matrix",
            &["transpose(matrix([1, 2], [3, 4]));  → matrix([1, 3], [2, 4])"], "matrix"));

        f.insert("eigenvalues", doc!("eigenvalues(mat)",
            "Computes the eigenvalues of a square matrix.",
            &["mat - square matrix"], "List of [eigenvalues, multiplicities]",
            &["eigenvalues(matrix([1, 2], [2, 1]));  → [[3, -1], [1, 1]]"], "matrix"));

        f.insert("eigenvectors", doc!("eigenvectors(mat)",
            "Computes both eigenvalues and eigenvectors of a square matrix.",
            &["mat - square matrix"], "List of eigenvalues and eigenvectors",
            &["eigenvectors(matrix([1, 2], [2, 1]));  → [[[3, -1], [1, 1]], [[[1, 1]], [[1, -1]]]]"], "matrix"));

        f.insert("charpoly", doc!("charpoly(mat, var)",
            "Computes the characteristic polynomial of a matrix.",
            &["mat - square matrix", "var - variable for the polynomial"], "Characteristic polynomial",
            &["charpoly(matrix([1, 2], [3, 4]), x);  → x^2 - 5*x - 2"], "matrix"));

        f.insert("echelon", doc!("echelon(mat)",
            "Computes the row echelon form of a matrix.",
            &["mat - matrix"], "Row echelon form", &[], "matrix"));

        f.insert("rank", doc!("rank(mat)",
            "Computes the rank of a matrix.",
            &["mat - matrix"], "Rank", &[], "matrix"));

        f.insert("nullspace", doc!("nullspace(mat)",
            "Computes the nullspace (kernel) of a matrix.",
            &["mat - matrix"], "Basis for nullspace", &[], "matrix"));

        f.insert("lu_factor", doc!("lu_factor(mat)",
            "Computes the LU factorization of a matrix.",
            &["mat - matrix"], "[LU, perm, lower-cases?]", &[], "matrix"));

        f.insert("cholesky", doc!("cholesky(mat)",
            "Computes the Cholesky decomposition of a symmetric positive-definite matrix.",
            &["mat - symmetric positive-definite matrix"], "Cholesky factor", &[], "matrix"));

        f.insert("ident", doc!("ident(n)",
            "Creates an n×n identity matrix.",
            &["n - size"], "Identity matrix",
            &["ident(3);  → matrix([1, 0, 0], [0, 1, 0], [0, 0, 1])"], "matrix"));

        f.insert("zeromatrix", doc!("zeromatrix(m, n)",
            "Creates an m×n zero matrix.",
            &["m - rows", "n - columns"], "Zero matrix", &[], "matrix"));

        f.insert("diagmatrix", doc!("diagmatrix(n, x)",
            "Creates an n×n diagonal matrix with x on the diagonal.",
            &["n - size", "x - diagonal element"], "Diagonal matrix", &[], "matrix"));

        f.insert("mattrace", doc!("mattrace(mat)",
            "Computes the trace (sum of diagonal elements) of a matrix.",
            &["mat - square matrix"], "Trace", &[], "matrix"));

        f.insert("addrow", doc!("addrow(mat, row1, row2, ...)",
            "Appends rows to a matrix.",
            &["mat - matrix", "rows - rows to append"], "Extended matrix", &[], "matrix"));

        f.insert("addcol", doc!("addcol(mat, col1, col2, ...)",
            "Appends columns to a matrix.",
            &["mat - matrix", "cols - columns to append"], "Extended matrix", &[], "matrix"));

        f.insert("submatrix", doc!("submatrix(mat, i1, i2, ..., j1, j2, ...)",
            "Extracts a submatrix by removing specified rows and columns.",
            &["mat - matrix", "i1, i2, ... - row indices to remove", "j1, j2, ... - column indices to remove"],
            "Submatrix", &[], "matrix"));

        f.insert("copymatrix", doc!("copymatrix(mat)",
            "Creates a copy of a matrix.",
            &["mat - matrix"], "Copy of matrix", &[], "matrix"));

        f.insert("vect", doc!("vect(expr)",
            "Declares an expression to be a vector for display purposes.",
            &["expr - expression to display as vector"], "Vector display", &[], "matrix"));

        // ═══════════════════════════════════════════════════
        // SETS
        // ═══════════════════════════════════════════════════
        f.insert("setify", doc!("setify(list)", "Converts a list to a set (removes duplicates, sorts).",
            &["list - list to convert"], "Set", &["setify([1, 2, 1, 3]);  → {1, 2, 3}"], "sets"));

        f.insert("setp", doc!("setp(expr)", "Returns true if expr is a Maxima set.",
            &["expr - expression to test"], "Boolean", &[], "sets"));

        f.insert("cardinality", doc!("cardinality(set)", "Returns the number of elements in a set.",
            &["set - a set"], "Integer", &["cardinality({1, 2, 3});  → 3"], "sets"));

        f.insert("elementp", doc!("elementp(x, set)", "Returns true if x is an element of set.",
            &["x - element", "set - a set"], "Boolean", &[], "sets"));

        f.insert("subsetp", doc!("subsetp(set1, set2)", "Returns true if set1 is a subset of set2.",
            &["set1, set2 - sets"], "Boolean", &[], "sets"));

        f.insert("union", doc!("union(set1, set2, ...)", "Returns the union of multiple sets.",
            &["set1, set2, ... - sets"], "Union set",
            &["union({1, 2, 3}, {3, 4, 5});  → {1, 2, 3, 4, 5}"], "sets"));

        f.insert("intersection", doc!("intersection(set1, set2, ...)", "Returns the intersection of multiple sets.",
            &["set1, set2, ... - sets"], "Intersection set",
            &["intersection({1, 2, 3}, {3, 4, 5});  → {3}"], "sets"));

        f.insert("setdifference", doc!("setdifference(set1, set2)", "Returns elements in set1 but not in set2.",
            &["set1, set2 - sets"], "Set difference",
            &["setdifference({1, 2, 3}, {2, 4});  → {1, 3}"], "sets"));

        f.insert("symmdifference", doc!("symmdifference(set1, set2)", "Returns the symmetric difference of two sets.",
            &["set1, set2 - sets"], "Symmetric difference", &[], "sets"));

        f.insert("makeset", doc!("makeset(expr, var, list)", "Generates a set by evaluating expr for each element of the list.",
            &["expr - expression in var", "var - variable", "list - list of values"], "Generated set", &[], "sets"));

        f.insert("adjoin", doc!("adjoin(x, set)", "Returns a set with element x added. No effect if x is already in set.",
            &["x - element to add", "set - target set"], "Updated set",
            &["adjoin(5, {1, 2, 3});  → {1, 2, 3, 5}"], "sets"));

        f.insert("disjoin", doc!("disjoin(x, set)", "Returns a set with element x removed. No effect if x is not in set.",
            &["x - element to remove", "set - target set"], "Updated set",
            &["disjoin(2, {1, 2, 3});  → {1, 3}"], "sets"));

        f.insert("powerset", doc!("powerset(set)", "Returns the power set of set (all subsets).",
            &["set - a set"], "Power set of set", &[], "sets"));

        f.insert("permutations", doc!("permutations(set)", "Returns a set of all permutations of the elements of set.",
            &["set - a set"], "Set of permutations", &[], "sets"));

        f.insert("random_permutation", doc!("random_permutation(set)", "Returns a random permutation of set.",
            &["set - a set or list"], "Random permutation", &[], "sets"));

        // ═══════════════════════════════════════════════════
        // NUMBER THEORY
        // ═══════════════════════════════════════════════════
        f.insert("primep", doc!("primep(n)", "Probabilistic primality test. Returns true if n is probably prime.",
            &["n - integer"], "Boolean", &["primep(17);  → true", "primep(100);  → false"], "number theory"));

        f.insert("next_prime", doc!("next_prime(n)", "Returns the smallest prime > n.",
            &["n - integer"], "Next prime", &["next_prime(17);  → 19"], "number theory"));

        f.insert("prev_prime", doc!("prev_prime(n)", "Returns the largest prime < n.",
            &["n - integer"], "Previous prime", &["prev_prime(17);  → 13"], "number theory"));

        f.insert("primes", doc!("primes(lo, hi)", "Returns a list of all primes in the interval [lo, hi].",
            &["lo - lower bound", "hi - upper bound"], "List of primes", &[], "number theory"));

        f.insert("factor", doc!("factor(n)", "Prime factorization of an integer n.",
            &["n - integer"], "Prime factorization",
            &["factor(60);  → 2^2*3*5", "factor(123456789);  → 3^2*3607*3803"], "number theory"));

        f.insert("divisors", doc!("divisors(n)", "Returns a set of all divisors of n.",
            &["n - integer"], "Set of divisors", &[], "number theory"));

        f.insert("divsum", doc!("divsum(n, k)", "Returns the sum of the k-th powers of the divisors of n.",
            &["n - integer", "k - power (default 1)"], "Sum of divisor powers", &[], "number theory"));

        f.insert("totient", doc!("totient(n)", "Euler's totient function: number of integers 1 <= k <= n with gcd(k,n)=1.",
            &["n - integer"], "Phi(n)", &[], "number theory"));

        f.insert("jacobi", doc!("jacobi(a, n)", "Jacobi symbol (a/n) for integer a and odd positive n.",
            &["a - integer", "n - odd positive integer"], "Jacobi symbol (-1, 0, or 1)", &[], "number theory"));

        f.insert("legendre", doc!("legendre(a, p)", "Legendre symbol (a/p) for integer a and odd prime p.",
            &["a - integer", "p - odd prime"], "Legendre symbol (-1, 0, or 1)", &[], "number theory"));

        f.insert("mod", doc!("mod(a, b)", "Modulus function: remainder of a divided by b (non-negative).",
            &["a - integer", "b - positive integer"], "Remainder",
            &["mod(17, 5);  → 2", "mod(-3, 5);  → 2"], "number theory"));

        f.insert("power_mod", doc!("power_mod(a, n, m)", "Computes a^n mod m efficiently.",
            &["a - base", "n - exponent", "m - modulus"], "a^n mod m", &[], "number theory"));

        f.insert("inv_mod", doc!("inv_mod(a, m)", "Computes the modular inverse of a modulo m.",
            &["a - integer", "m - modulus"], "Modular inverse", &[], "number theory"));

        f.insert("chinese", doc!("chinese([rem1, rem2, ...], [mod1, mod2, ...])",
            "Solves the Chinese Remainder Theorem system x ≡ remi (mod modi).",
            &["rems - remainders", "mods - moduli"], "Solution x",
            &["chinese([2, 3], [3, 5]);  → 8"], "number theory"));

        f.insert("fibonacci", doc!("fibonacci(n)", "Returns the n-th Fibonacci number (F(0)=0, F(1)=1).",
            &["n - integer"], "Fibonacci number",
            &["fibonacci(10);  → 55"], "number theory"));

        f.insert("bernoulli", doc!("bernoulli(n)", "Returns the n-th Bernoulli number.",
            &["n - non-negative integer"], "Bernoulli number", &[], "number theory"));

        f.insert("random", doc!("random(n)", "Returns a random integer between 0 and n-1 (inclusive). With x argument, returns a random float.",
            &["n - integer (or float)"], "Random number", &["random(10);  → 0..9"], "number theory"));

        f.insert("isqrt", doc!("isqrt(n)", "Returns the integer square root of n (floor of sqrt(n)).",
            &["n - non-negative integer"], "Integer square root", &["isqrt(17);  → 4"], "number theory"));

        // ═══════════════════════════════════════════════════
        // SPECIAL FUNCTIONS
        // ═══════════════════════════════════════════════════
        f.insert("sqrt", doc!("sqrt(x)", "Square root of x. Returns a simplified radical form.",
            &["x - expression"], "Square root",
            &["sqrt(16);  → 4", "sqrt(x^2);  → |x|"], "special"));

        f.insert("abs", doc!("abs(x)", "Absolute value of x.",
            &["x - expression"], "Absolute value",
            &["abs(-5);  → 5", "abs(x^2);  → x^2"], "special"));

        f.insert("log", doc!("log(x)", "Natural logarithm of x (base e).",
            &["x - expression (> 0)"], "Natural logarithm",
            &["log(%e);  → 1", "log(1);  → 0", "log(x^2);  → 2*log(x)"], "special"));

        f.insert("exp", doc!("exp(x)", "Exponential function: e^x.",
            &["x - expression"], "e^x",
            &["exp(0);  → 1", "exp(1);  → %e", "exp(x)*exp(y);  → exp(x + y)"], "special"));

        f.insert("gamma", doc!("gamma(z)", "Gamma function Γ(z). For integer n, gamma(n+1) = n!.",
            &["z - expression"], "Gamma value",
            &["gamma(5);  → 24", "gamma(1/2);  → sqrt(%pi)"], "special"));

        f.insert("log_gamma", doc!("log_gamma(z)", "Natural logarithm of the gamma function: log(Γ(z)).",
            &["z - expression"], "log(Γ(z))", &[], "special"));

        f.insert("beta", doc!("beta(a, b)", "Beta function B(a, b) = Γ(a)Γ(b)/Γ(a+b).",
            &["a, b - expressions"], "Beta value", &[], "special"));

        f.insert("zeta", doc!("zeta(z)", "Riemann zeta function ζ(z).",
            &["z - expression"], "Zeta value",
            &["zeta(2);  → %pi^2/6", "zeta(0);  → -1/2"], "special"));

        f.insert("erf", doc!("erf(x)", "Error function: (2/√π)∫₀ˣ exp(-t²) dt.",
            &["x - expression"], "Error function value",
            &["erf(0);  → 0", "erf(inf);  → 1"], "special"));

        f.insert("erfc", doc!("erfc(x)", "Complementary error function: 1 - erf(x).",
            &["x - expression"], "erfc(x)", &[], "special"));

        f.insert("bessel_j", doc!("bessel_j(n, x)", "Bessel function of the first kind J_n(x).",
            &["n - order", "x - argument"], "J_n(x)", &[], "special"));

        f.insert("bessel_y", doc!("bessel_y(n, x)", "Bessel function of the second kind Y_n(x).",
            &["n - order", "x - argument"], "Y_n(x)", &[], "special"));

        f.insert("bessel_i", doc!("bessel_i(n, x)", "Modified Bessel function of the first kind I_n(x).",
            &["n - order", "x - argument"], "I_n(x)", &[], "special"));

        f.insert("bessel_k", doc!("bessel_k(n, x)", "Modified Bessel function of the second kind K_n(x).",
            &["n - order", "x - argument"], "K_n(x)", &[], "special"));

        f.insert("hankel_1", doc!("hankel_1(n, x)", "Hankel function of the first kind H(1)_n(x).",
            &["n - order", "x - argument"], "H(1)_n(x)", &[], "special"));

        f.insert("hankel_2", doc!("hankel_2(n, x)", "Hankel function of the second kind H(2)_n(x).",
            &["n - order", "x - argument"], "H(2)_n(x)", &[], "special"));

        f.insert("airy_ai", doc!("airy_ai(x)", "Airy function Ai(x).", &["x - expression"], "Ai(x)", &[], "special"));

        f.insert("airy_bi", doc!("airy_bi(x)", "Airy function Bi(x).", &["x - expression"], "Bi(x)", &[], "special"));

        f.insert("elliptic_kc", doc!("elliptic_kc(m)", "Complete elliptic integral of the first kind K(m).",
            &["m - parameter"], "K(m)", &[], "special"));

        f.insert("elliptic_ec", doc!("elliptic_ec(m)", "Complete elliptic integral of the second kind E(m).",
            &["m - parameter"], "E(m)", &[], "special"));

        f.insert("elliptic_f", doc!("elliptic_f(phi, m)", "Incomplete elliptic integral of the first kind F(φ, m).",
            &["phi - amplitude", "m - parameter"], "F(φ, m)", &[], "special"));

        f.insert("elliptic_e", doc!("elliptic_e(phi, m)", "Incomplete elliptic integral of the second kind E(φ, m).",
            &["phi - amplitude", "m - parameter"], "E(φ, m)", &[], "special"));

        f.insert("expintegral_e", doc!("expintegral_e(n, x)", "Exponential integral E_n(x).",
            &["n - order", "x - argument"], "E_n(x)", &[], "special"));

        f.insert("expintegral_ei", doc!("expintegral_ei(x)", "Exponential integral Ei(x).",
            &["x - argument"], "Ei(x)", &[], "special"));

        f.insert("expintegral_si", doc!("expintegral_si(x)", "Sine integral Si(x).",
            &["x - argument"], "Si(x)", &[], "special"));

        f.insert("expintegral_ci", doc!("expintegral_ci(x)", "Cosine integral Ci(x).",
            &["x - argument"], "Ci(x)", &[], "special"));

        f.insert("psi", doc!("psi[n](x)", "Polygamma function: n-th derivative of digamma ψ(n)(x). psi[0](x) is digamma.",
            &["n - order (subscript)", "x - argument"], "Polygamma value", &[], "special"));

        f.insert("hermite", doc!("hermite(n, x)", "Hermite polynomial H_n(x).",
            &["n - degree", "x - variable"], "Hermite polynomial", &[], "special"));

        f.insert("gen_laguerre", doc!("gen_laguerre(n, a, x)", "Generalized Laguerre polynomial L_n^a(x).",
            &["n - degree", "a - parameter", "x - variable"], "Laguerre polynomial", &[], "special"));

        f.insert("legendre_p", doc!("legendre_p(n, x)", "Legendre polynomial of the first kind P_n(x).",
            &["n - degree", "x - variable"], "Legendre polynomial", &[], "special"));

        f.insert("legendre_q", doc!("legendre_q(n, x)", "Legendre function of the second kind Q_n(x).",
            &["n - degree", "x - variable"], "Legendre function", &[], "special"));

        f.insert("chebyshev_t", doc!("chebyshev_t(n, x)", "Chebyshev polynomial of the first kind T_n(x).",
            &["n - degree", "x - variable"], "Chebyshev polynomial", &[], "special"));

        f.insert("chebyshev_u", doc!("chebyshev_u(n, x)", "Chebyshev polynomial of the second kind U_n(x).",
            &["n - degree", "x - variable"], "Chebyshev polynomial", &[], "special"));

        f.insert("spherical_harmonic", doc!("spherical_harmonic(l, m, theta, phi)",
            "Spherical harmonic function Y_l^m(θ, φ).",
            &["l - degree", "m - order", "theta - polar angle", "phi - azimuthal angle"],
            "Spherical harmonic", &[], "special"));

        // ═══════════════════════════════════════════════════
        // CONTROL FLOW / PROGRAMMING
        // ═══════════════════════════════════════════════════
        f.insert("block", doc!("block([v1, v2, ...], expr1, expr2, ...)",
            "Compound expression with optional local variables. Evaluates expressions in sequence, returns the last value.",
            &["v1, v2, ... - local variable names (optional, can be initialized with v:val)", "expr1, expr2, ... - expressions to evaluate"],
            "Value of the last expression",
            &["block([x: 1, y: 2], x + y);  → 3", "block(print('hello'), print('world'));"], "control"));

        f.insert("lambda", doc!("lambda([v1, v2, ...], expr1, expr2, ...)",
            "Creates an anonymous function with formal parameters v1, v2, ... and body expr1..exprn.",
            &["v1, v2, ... - formal parameter names", "expr1, expr2, ... - body expressions"],
            "Lambda expression (function)",
            &["lambda([x], x^2);  → lambda([x], x^2)", "map(lambda([x], x^2), [1, 2, 3]);  → [1, 4, 9]"], "control"));

        f.insert("define", doc!("define(f(x, ...), expr)",
            "Defines a function f(x,...) with body expr, but EVALUATES expr at definition time (unlike :=).",
            &["f(x, ...) - function name and parameters", "expr - body (evaluated at definition time)"],
            "Function definition",
            &["define(f(x), expand((x+1)^2));  → f(x) := x^2 + 2*x + 1"], "control"));

        f.insert("buildq", doc!("buildq([v1, v2, ...], template)",
            "Builds an expression by substituting values into a template, with optional splicing via splice().",
            &["vars - variable list with values", "template - expression template with substitutions"],
            "Constructed expression", &[], "control"));

        f.insert("splice", doc!("splice(list)", "Used within buildq to splice a list into an expression.", &[], "Spliced expression", &[], "control"));

        f.insert("apply", doc!("apply(f, args)",
            "Applies a function f to a list of arguments. f can be a function name or lambda.",
            &["f - function name or lambda", "args - list of arguments"],
            "Result of function application",
            &["apply('+', [1, 2, 3]);  → 6", "apply(lambda([x], x^2), [5]);  → 25"], "control"));

        f.insert("map", doc!("map(f, expr1, expr2, ...)",
            "Applies function f to each element of expr1, expr2, ... in parallel.",
            &["f - function name or lambda", "expr1, expr2, ... - expressions"],
            "Expression with f applied elementwise",
            &["map(sin, [0, %pi/2, %pi]);  → [0, 1, 0]", "map('+, [1, 2], [3, 4]);  → [4, 6]"], "control"));

        f.insert("scanmap", doc!("scanmap(f, expr)", "Recursively applies f to all subexpressions of expr.", &[], "Scanned expression", &[], "control"));

        f.insert("fullmap", doc!("fullmap(f, expr)", "Similar to map but recurses into all subexpressions.", &[], "Result", &[], "control"));

        f.insert("maplist", doc!("maplist(f, expr1, expr2, ...)", "Like map but always returns a list.", &[], "List result", &[], "control"));

        f.insert("makelist", doc!("makelist(expr, var, lo, hi)",
            "Creates a list by evaluating expr for var = lo, lo+1, ..., hi.",
            &["expr - expression in var", "var - index variable", "lo - lower bound", "hi - upper bound"],
            "Generated list",
            &["makelist(k^2, k, 1, 5);  → [1, 4, 9, 16, 25]"], "control"));

        f.insert("create_list", doc!("create_list(expr, var1, list1, var2, list2, ...)",
            "Creates a list by nesting makelist operations for multiple variables.",
            &["expr - expression", "var - variable", "list - list of values"],
            "Generated list", &[], "control"));

        f.insert("throw", doc!("throw(expr)", "Throws a non-local exit, to be caught by catch.", &[], "None (triggers non-local exit)", &[], "control"));

        f.insert("catch", doc!("catch(expr1, expr2, ...)",
            "Evaluates expressions. If throw is called during evaluation, catch catches it.",
            &["expr1, expr2, ... - expressions"], "Value of last expr or thrown value",
            &["catch(throw(42));  → 42"], "control"));

        f.insert("error", doc!("error(msg1, msg2, ...)",
            "Signals an error with the given message(s).",
            &["msgs - message strings or expressions"], "None (signals error)", &[], "control"));

        f.insert("errcatch", doc!("errcatch(expr1, expr2, ...)",
            "Evaluates expressions. If an error occurs, returns [] instead of propagating.",
            &["expr1, expr2, ..."], "List of results or [] on error", &[], "control"));

        f.insert("return", doc!("return(value)",
            "Returns a value from a block or function (non-local exit).",
            &["value - value to return"], "The value (non-local return)", &[], "control"));

        // ═══════════════════════════════════════════════════
        // LIST OPERATIONS
        // ═══════════════════════════════════════════════════
        f.insert("cons", doc!("cons(expr, list)",
            "Inserts expr at the beginning of list.",
            &["expr - element to insert", "list - target list"],
            "New list",
            &["cons(1, [2, 3]);  → [1, 2, 3]"], "lists"));

        f.insert("endcons", doc!("endcons(expr, list)",
            "Inserts expr at the end of list.",
            &["expr - element to append", "list - target list"],
            "New list",
            &["endcons(3, [1, 2]);  → [1, 2, 3]"], "lists"));

        f.insert("append", doc!("append(list1, list2, ...)",
            "Merges multiple lists into a single list.",
            &["list1, list2, ... - lists to merge"],
            "Merged list",
            &["append([1, 2], [3, 4]);  → [1, 2, 3, 4]"], "lists"));

        f.insert("reverse", doc!("reverse(list)",
            "Reverses the order of elements in a list.",
            &["list - list to reverse"], "Reversed list",
            &["reverse([1, 2, 3]);  → [3, 2, 1]"], "lists"));

        f.insert("member", doc!("member(expr, list)",
            "Returns true if expr is an element of list.",
            &["expr - element to search", "list - list to search"], "Boolean",
            &["member(2, [1, 2, 3]);  → true"], "lists"));

        f.insert("delete", doc!("delete(expr, list, n)",
            "Deletes all occurrences of expr from list. If n specified, deletes at most n occurrences.",
            &["expr - element to delete", "list - list to modify", "n - max deletions (optional)"],
            "List with deletions",
            &["delete(2, [1, 2, 3, 2]);  → [1, 3]"], "lists"));

        f.insert("last", doc!("last(list)",
            "Returns the last element of list.",
            &["list - list"], "Last element",
            &["last([1, 2, 3]);  → 3"], "lists"));

        f.insert("first", doc!("first(list)",
            "Returns the first element of list.",
            &["list - list"], "First element",
            &["first([1, 2, 3]);  → 1"], "lists"));

        f.insert("rest", doc!("rest(list, n)",
            "Returns list without the first n elements. If n negative, drops last |n| elements.",
            &["list - list", "n - number to drop (default 1)"], "Sublist",
            &["rest([1, 2, 3]);  → [2, 3]", "rest([1, 2, 3], 2);  → [3]"], "lists"));

        f.insert("length", doc!("length(expr)",
            "Returns the number of elements of expr (list, set, matrix, or general expression).",
            &["expr - expression"], "Length",
            &["length([1, 2, 3]);  → 3"], "lists"));

        f.insert("flatten", doc!("flatten(expr)",
            "Recursively flattens all list levels into a single list.",
            &["expr - nested expression"], "Flattened list",
            &["flatten([1, [2, [3]]]);  → [1, 2, 3]"], "lists"));

        f.insert("sort", doc!("sort(list, pred)",
            "Sorts a list. Without pred, natural order. With pred, uses the comparison predicate.",
            &["list - list to sort", "pred - comparison predicate (optional)"],
            "Sorted list",
            &["sort([3, 1, 2]);  → [1, 2, 3]"], "lists"));

        f.insert("unique", doc!("unique(list)",
            "Returns a list with duplicate elements removed (preserves order of first occurrence).",
            &["list - list with potential duplicates"], "De-duplicated list", &[], "lists"));

        f.insert("assoc", doc!("assoc(key, list)",
            "Searches a list of pairs for a pair with the given key.",
            &["key - key to search", "list - list of pairs"], "Matching pair or false", &[], "lists"));

        f.insert("sublist", doc!("sublist(list, pred)",
            "Returns a list of elements of list satisfying the predicate pred.",
            &["list - list", "pred - boolean predicate"], "Filtered list",
            &["sublist([1, 2, 3, 4], lambda([x], x > 2));  → [3, 4]"], "lists"));

        f.insert("every", doc!("every(pred, list)",
            "Returns true if pred returns true for every element in list.",
            &["pred - predicate function", "list - list"], "Boolean", &[], "lists"));

        f.insert("some", doc!("some(pred, list)",
            "Returns true if pred returns true for at least one element in list.",
            &["pred - predicate function", "list - list"], "Boolean", &[], "lists"));

        f.insert("lreduce", doc!("lreduce(f, list)", "Left-associative binary reduction of list using f.", &[], "Reduced value", &[], "lists"));
        f.insert("rreduce", doc!("rreduce(f, list)", "Right-associative binary reduction of list using f.", &[], "Reduced value", &[], "lists"));
        f.insert("xreduce", doc!("xreduce(f, list)", "Binary reduction of list using f (divide and conquer).", &[], "Reduced value", &[], "lists"));

        // ═══════════════════════════════════════════════════
        // DISPLAY / OUTPUT
        // ═══════════════════════════════════════════════════
        f.insert("print", doc!("print(expr1, expr2, ...)",
            "Prints expressions, separated by spaces. Returns the last expression.",
            &["expr1, expr2, ... - expressions to print"], "Last expression printed",
            &["print('hello, world');", "print('x =', x);"], "display"));

        f.insert("display", doc!("display(expr1, expr2, ...)",
            "Displays expressions labeled with their names.",
            &["expr1, expr2, ... - expressions to display"], "List of displayed expressions",
            &["display(x, y);"], "display"));

        f.insert("tex", doc!("tex(expr, dest)",
            "Outputs expression in TeX format. If dest is given, writes to file.",
            &["expr - expression to convert", "dest - optional filename or stream"],
            "TeX string or file output",
            &["tex(integrate(sin(x), x));  → -\\cos x"], "display"));

        f.insert("fortran", doc!("fortran(expr)",
            "Outputs expression in Fortran format.",
            &["expr - expression"], "Fortran representation", &[], "display"));

        f.insert("c", doc!("c(expr)",
            "Outputs expression in C format.",
            &["expr - expression"], "C representation", &[], "display"));

        f.insert("string", doc!("string(expr)",
            "Converts expression to a Maxima string representation.",
            &["expr - expression"], "String",
            &["string(x^2 + 1);  → x^2+1"], "display"));

        f.insert("concat", doc!("concat(expr1, expr2, ...)",
            "Concatenates expressions into a string.",
            &["expr1, expr2, ... - expressions"], "Concatenated string",
            &["concat('value = ', 42);  → value = 42"], "display"));

        f.insert("printf", doc!("printf(dest, format, arg1, arg2, ...)",
            "Formatted output like C printf. dest can be a stream, file, or true for stdout.",
            &["dest - destination (true=stdout)", "format - format string", "args - arguments"],
            "Formatted output", &[], "display"));

        f.insert("grind", doc!("grind(expr)",
            "Outputs expression in a form suitable for re-input to Maxima.",
            &["expr - expression"], "Grind string", &[], "display"));

        f.insert("disp", doc!("disp(expr1, expr2, ...)", "Displays expressions centered without labels.", &[], "Display output", &[], "display"));

        // ═══════════════════════════════════════════════════
        // SYSTEM / FILE I/O
        // ═══════════════════════════════════════════════════
        f.insert("kill", doc!("kill(a, b, ...)  |  kill(all)",
            "Removes variables, functions, arrays, labels, etc. kill(all) removes everything.",
            &["names - symbols to remove (all removes everything)"],
            "done",
            &["kill(x);  kill(all);"], "system"));

        f.insert("remvalue", doc!("remvalue(a, b, ...)", "Removes the values of specified variables.",
            &["vars - variables to unassign"], "done", &[], "system"));

        f.insert("remfunction", doc!("remfunction(f1, f2, ...)", "Removes function definitions.",
            &["functions - functions to remove"], "done", &[], "system"));

        f.insert("remrule", doc!("remrule(func, rule)", "Removes a simplification rule from a function.",
            &["func - function name", "rule - rule name or 'all'"], "done", &[], "system"));

        f.insert("load", doc!("load(filename)",
            "Loads a Maxima file or package. Searches the file search path.",
            &["filename - file name (with or without .mac/.lisp extension)"],
            "Path of loaded file",
            &["load('eigen');", "load('myfile.mac');"], "system"));

        f.insert("batch", doc!("batch(filename)",
            "Loads and executes a Maxima file, printing input and output.",
            &["filename - file to load"], "done", &[], "system"));

        f.insert("batchload", doc!("batchload(filename)",
            "Loads a file without printing input or output.",
            &["filename - file to load"], "done", &[], "system"));

        f.insert("save", doc!("save(filename, a, b, ...)  |  save(filename, all)",
            "Saves specified symbols to a file in LISP format.",
            &["filename - output file", "names - symbols to save (or 'all')"],
            "Filename", &[], "system"));

        f.insert("stringout", doc!("stringout(filename, expr1, expr2, ...)",
            "Writes expressions to a file as strings.",
            &["filename - output file", "exprs - expressions to write"], "done", &[], "system"));

        f.insert("writefile", doc!("writefile(filename)",
            "Begins writing all console output to filename.",
            &["filename - output file"], "done", &[], "system"));

        f.insert("closefile", doc!("closefile()",
            "Closes the file opened by writefile.", &[], "done", &[], "system"));

        f.insert("system", doc!("system(cmd)",
            "Executes an external shell command.",
            &["cmd - shell command string"], "Exit code", &[], "system"));

        f.insert("runtime", doc!("runtime()",
            "Returns the elapsed time since the Maxima session started, in milliseconds.",
            &[], "Time in milliseconds", &[], "system"));

        f.insert("elapsed", doc!("elapsed()",
            "Returns the elapsed real time since the last call to elapsed().",
            &[], "Elapsed time in seconds", &[], "system"));

        f.insert("timestamp", doc!("timestamp()",
            "Returns a string with the current date and time.",
            &[], "Date/time string", &[], "system"));

        f.insert("history", doc!("history(n)",
            "Displays the last n input and output lines.",
            &["n - number of lines to display (optional)"], "History display", &[], "system"));

        f.insert("describe", doc!("describe(topic)",
            "Displays documentation for a topic.",
            &["topic - function name or concept"], "Documentation text", &[], "system"));

        f.insert("apropos", doc!("apropos(str)",
            "Returns a list of functions and variables containing str in their name.",
            &["str - search string"], "List of matching symbols", &[], "system"));

        f.insert("build_info", doc!("build_info()",
            "Returns a list with Maxima version and build information.",
            &[], "Build info list", &[], "system"));

        f.insert("features", doc!("features()",
            "Returns a list of system features.",
            &[], "List of features", &[], "system"));

        f.insert("eval_string", doc!("eval_string(str)",
            "Parses and evaluates str as a Maxima expression.",
            &["str - string to evaluate"], "Evaluation result", &[], "system"));

        f.insert("parse_string", doc!("parse_string(str)",
            "Parses str as a Maxima expression without evaluating it.",
            &["str - string to parse"], "Parsed expression", &[], "system"));

        f.insert("read", doc!("read(expr1, expr2, ...)",
            "Reads and evaluates an expression from stdin. Optional prompts.",
            &["exprs - optional prompt expressions"], "Read and evaluated expression", &[], "system"));

        // ═══════════════════════════════════════════════════
        // NUMERICAL METHODS
        // ═══════════════════════════════════════════════════
        f.insert("find_root", doc!("find_root(expr, var, lo, hi)  |  find_root(f, lo, hi)",
            "Finds a root of an expression in the interval [lo, hi] using Brent's method.",
            &["expr - expression or function name", "var - variable (if using expression)", "lo - lower bound", "hi - upper bound"],
            "Root value",
            &["find_root(sin(x), x, 3, 4);  → 3.141592653589793"], "numerical"));

        f.insert("newton", doc!("newton(expr, var, guess)",
            "Finds a root using Newton's method with initial guess.",
            &["expr - expression", "var - variable", "guess - initial guess"],
            "Root value", &[], "numerical"));

        f.insert("mnewton", doc!("mnewton([eqns], [vars], [guesses])",
            "Numerically solves a system of equations using Newton's method.",
            &["eqns - list of equations", "vars - list of variables", "guesses - list of initial guesses"],
            "List of solutions", &[], "numerical"));

        f.insert("rk", doc!("rk(expr, var, init, interval)",
            "Fourth-order Runge-Kutta method for ODEs.",
            &["expr - derivative expression", "var - dependent variable", "init - initial value", "interval - [var, lo, hi, step]"],
            "List of [var, val] pairs", &[], "numerical"));

        f.insert("romberg", doc!("romberg(expr, var, lo, hi)",
            "Numerically integrates an expression using Romberg's method.",
            &["expr - expression to integrate", "var - variable", "lo - lower limit", "hi - upper limit"],
            "Numerical integral", &[], "numerical"));

        f.insert("float", doc!("float(expr)",
            "Converts rational numbers and bigfloats to double-precision floats.",
            &["expr - expression"], "Expression with numeric approximations",
            &["float(1/3);  → 0.3333333333333333", "float(sqrt(2));  → 1.414213562373095"], "numerical"));

        f.insert("bfloat", doc!("bfloat(expr)",
            "Converts numbers to bigfloats (arbitrary precision).",
            &["expr - expression"], "Bigfloat expression",
            &["bfloat(%pi);  → 3.141592653589793b0"], "numerical"));

        f.insert("numer", doc!("numer", "When set to true, arithmetic is evaluated numerically.", &[], "Boolean flag", &[], "numerical"));

        f.insert("fpprec", doc!("fpprec: n",
            "Controls the precision (number of digits) for bigfloat arithmetic.",
            &["n - number of digits (default: 16)"], "Precision setting",
            &["fpprec: 50; bfloat(%pi);"], "numerical"));

        // ═══════════════════════════════════════════════════
        // PLOTTING
        // ═══════════════════════════════════════════════════
        f.insert("plot2d", doc!("plot2d(expr, [var, lo, hi], options)",
            "2D plot of one or more functions.",
            &["expr - function(s) to plot (list for multiple)", "[var, lo, hi] - variable range", "options - optional plot options"],
            "Plot displayed", &[], "plotting"));

        f.insert("plot3d", doc!("plot3d(expr, [x, xlo, xhi], [y, ylo, yhi], options)",
            "3D surface plot.",
            &["expr - function f(x,y)", "[x, xlo, xhi] - x range", "[y, ylo, yhi] - y range", "options - optional plot options"],
            "Plot displayed", &[], "plotting"));

        f.insert("contour_plot", doc!("contour_plot(expr, [x, xlo, xhi], [y, ylo, yhi])",
            "Contour plot of a function of two variables.",
            &["expr - function f(x,y)", "x range", "y range"], "Contour plot", &[], "plotting"));

        f.insert("implicit_plot", doc!("implicit_plot([eqns], [x, xlo, xhi], [y, ylo, yhi])",
            "Plots implicit equations (curves defined by f(x,y)=0).",
            &["eqns - equation(s) to plot", "x range", "y range"], "Implicit plot", &[], "plotting"));

        f.insert("wxplot2d", doc!("wxplot2d(expr, [var, lo, hi], options)",
            "2D plot displayed in wxMaxima interface.",
            &["same as plot2d"], "Plot displayed in wxMaxima", &[], "plotting"));

        f.insert("wxplot3d", doc!("wxplot3d(expr, [x, xlo, xhi], [y, ylo, yhi], options)",
            "3D plot displayed in wxMaxima interface.",
            &["same as plot3d"], "Plot displayed in wxMaxima", &[], "plotting"));

        // ═══════════════════════════════════════════════════
        // ARITHMETIC OPERATORS
        // ═══════════════════════════════════════════════════
        f.insert("+", doc!("a + b", "Addition operator.", &["a, b - expressions"], "Sum", &[], "arithmetic"));
        f.insert("-", doc!("a - b  |  -b", "Subtraction or negation operator.", &["a, b - expressions"], "Difference or negation", &[], "arithmetic"));
        f.insert("*", doc!("a * b", "Multiplication operator.", &["a, b - expressions"], "Product", &[], "arithmetic"));
        f.insert("/", doc!("a / b", "Division operator.", &["a, b - expressions"], "Quotient", &[], "arithmetic"));
        f.insert("^", doc!("a ^ b", "Exponentiation operator.", &["a - base", "b - exponent"], "a^b", &[], "arithmetic"));
        f.insert("^^", doc!("a ^^ b", "Non-commutative exponentiation (matrix exponent).", &["a - matrix", "b - exponent"], "a^^b", &[], "arithmetic"));
        f.insert(".", doc!("a . b", "Non-commutative multiplication (dot product / matrix product).", &["a, b - matrices or vectors"], "Matrix product", &[], "arithmetic"));
        f.insert("@", doc!("a @ b", "Property operator.", &[], "Property value", &[], "arithmetic"));

        // ═══════════════════════════════════════════════════
        // CONSTANTS
        // ═══════════════════════════════════════════════════
        let mut constants = HashMap::new();
        constants.insert("%",      doc!("%", "Last computed result in the session.", &[], "Previous result", &[], "constant"));
        constants.insert("%e",     doc!("%e", "Euler's number e ≈ 2.71828. Base of natural logarithms.", &[], "e", &[], "constant"));
        constants.insert("%i",     doc!("%i", "Imaginary unit: sqrt(-1).", &[], "%i", &["%i^2;  → -1"], "constant"));
        constants.insert("%pi",    doc!("%pi", "Pi (π) ≈ 3.14159265358979. Ratio of circumference to diameter.", &[], "π", &[], "constant"));
        constants.insert("%phi",   doc!("%phi", "Golden ratio φ ≈ 1.618033988749895.", &[], "φ", &[], "constant"));
        constants.insert("%gamma", doc!("%gamma", "Euler-Mascheroni constant γ ≈ 0.5772156649015329.", &[], "γ", &[], "constant"));
        constants.insert("inf",    doc!("inf", "Positive infinity.", &[], "∞", &[], "constant"));
        constants.insert("minf",   doc!("minf", "Negative infinity (-∞).", &[], "-∞", &[], "constant"));
        constants.insert("true",   doc!("true", "Boolean true value.", &[], "true", &[], "constant"));
        constants.insert("false",  doc!("false", "Boolean false value.", &[], "false", &[], "constant"));
        constants.insert("und",    doc!("und", "Undefined result (e.g., 0/0).", &[], "Undefined", &[], "constant"));
        constants.insert("ind",    doc!("ind", "Indefinite result (bounded but not unique).", &[], "Indefinite", &[], "constant"));

        // ═══════════════════════════════════════════════════
        // KEYWORDS & OPERATORS
        // ═══════════════════════════════════════════════════
        let keywords = vec![
            "if", "then", "else", "elseif",
            "for", "do", "while", "unless", "in",
            "step", "next", "thru", "block", "lambda",
            "not", "and", "or",
        ];

        let operators = vec![
            "+", "-", "*", "/", "^", "^^", ".", "@",
            "=", "#", "<", ">", "<=", ">=",
            ":", "::", ":=", "::=",
            "'", "''", "!", "!!",
        ];

        Self { functions: f, constants, keywords, operators }
    }
}
