use std::collections::HashMap;

pub struct Builtins {
    pub functions: HashMap<&'static str, DocEntry>,
    pub constants: HashMap<&'static str, DocEntry>,
    pub keywords: Vec<&'static str>,
    pub operators: Vec<&'static str>,
}

pub struct DocEntry {
    pub signature: &'static str,
    pub doc: &'static str,
    pub category: &'static str,
}

impl Builtins {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // ── Arithmetic ──
        functions.insert("+", DocEntry { signature: "+", doc: "Addition operator", category: "arithmetic" });
        functions.insert("-", DocEntry { signature: "-", doc: "Subtraction / negation operator", category: "arithmetic" });
        functions.insert("*", DocEntry { signature: "*", doc: "Multiplication operator", category: "arithmetic" });
        functions.insert("/", DocEntry { signature: "/", doc: "Division operator", category: "arithmetic" });
        functions.insert("^", DocEntry { signature: "^", doc: "Exponentiation operator", category: "arithmetic" });
        functions.insert("^^", DocEntry { signature: "^^", doc: "Non-commutative exponentiation / matrix exponent", category: "arithmetic" });
        functions.insert(".", DocEntry { signature: ".", doc: "Non-commutative multiplication / dot product", category: "arithmetic" });

        // ── Assignment ──
        functions.insert(":", DocEntry { signature: "var : expr", doc: "Assignment operator", category: "assignment" });

        // ── Trig ──
        let trig_signatures: [&'static str; 21] = [
            "sin(x)", "cos(x)", "tan(x)", "cot(x)", "sec(x)", "csc(x)",
            "asin(x)", "acos(x)", "atan(x)", "acot(x)", "asec(x)", "acsc(x)",
            "sinh(x)", "cosh(x)", "tanh(x)", "coth(x)", "sech(x)", "csch(x)",
            "asinh(x)", "acosh(x)", "atanh(x)",
        ];
        for (i, &(name, desc)) in [
            ("sin", "Sine"),
            ("cos", "Cosine"),
            ("tan", "Tangent"),
            ("cot", "Cotangent"),
            ("sec", "Secant"),
            ("csc", "Cosecant"),
            ("asin", "Arc sine"),
            ("acos", "Arc cosine"),
            ("atan", "Arc tangent"),
            ("acot", "Arc cotangent"),
            ("asec", "Arc secant"),
            ("acsc", "Arc cosecant"),
            ("sinh", "Hyperbolic sine"),
            ("cosh", "Hyperbolic cosine"),
            ("tanh", "Hyperbolic tangent"),
            ("coth", "Hyperbolic cotangent"),
            ("sech", "Hyperbolic secant"),
            ("csch", "Hyperbolic cosecant"),
            ("asinh", "Inverse hyperbolic sine"),
            ("acosh", "Inverse hyperbolic cosine"),
            ("atanh", "Inverse hyperbolic tangent"),
        ].iter().enumerate() {
            functions.insert(name, DocEntry {
                signature: trig_signatures[i],
                doc: desc,
                category: "trigonometry",
            });
        }

        // ── Calculus ──
        functions.insert("diff", DocEntry { signature: "diff(expr, var, n)", doc: "Differentiate expr with respect to var n times", category: "calculus" });
        functions.insert("integrate", DocEntry { signature: "integrate(expr, var)  |  integrate(expr, var, lo, hi)", doc: "Integrate expr with respect to var", category: "calculus" });
        functions.insert("sum", DocEntry { signature: "sum(expr, var, lo, hi)", doc: "Summation from var=lo to hi", category: "calculus" });
        functions.insert("product", DocEntry { signature: "product(expr, var, lo, hi)", doc: "Product from var=lo to hi", category: "calculus" });
        functions.insert("limit", DocEntry { signature: "limit(expr, var, val, dir)", doc: "Limit of expr as var approaches val from direction dir", category: "calculus" });
        functions.insert("taylor", DocEntry { signature: "taylor(expr, var, pt, n)", doc: "Taylor series expansion", category: "calculus" });
        functions.insert("series", DocEntry { signature: "series(expr, var, pt, n)", doc: "Series expansion", category: "calculus" });
        functions.insert("laplace", DocEntry { signature: "laplace(expr, t, s)", doc: "Laplace transform", category: "calculus" });
        functions.insert("ilt", DocEntry { signature: "ilt(expr, s, t)", doc: "Inverse Laplace transform", category: "calculus" });

        // ── Algebraic manipulation ──
        functions.insert("expand", DocEntry { signature: "expand(expr)", doc: "Expand an expression", category: "algebra" });
        functions.insert("factor", DocEntry { signature: "factor(expr)", doc: "Factor an expression", category: "algebra" });
        functions.insert("ratsimp", DocEntry { signature: "ratsimp(expr)", doc: "Simplify rational expression", category: "algebra" });
        functions.insert("fullratsimp", DocEntry { signature: "fullratsimp(expr)", doc: "Repeatedly simplify rational expression", category: "algebra" });
        functions.insert("radcan", DocEntry { signature: "radcan(expr)", doc: "Simplify radicals and logs", category: "algebra" });
        functions.insert("logcontract", DocEntry { signature: "logcontract(expr)", doc: "Contract logarithms", category: "algebra" });
        functions.insert("rootscontract", DocEntry { signature: "rootscontract(expr)", doc: "Contract roots", category: "algebra" });
        functions.insert("partfrac", DocEntry { signature: "partfrac(expr, var)", doc: "Partial fraction decomposition", category: "algebra" });
        functions.insert("subst", DocEntry { signature: "subst(x, y, expr)", doc: "Substitute x for y in expr", category: "algebra" });
        functions.insert("psubst", DocEntry { signature: "psubst(list, expr)", doc: "Parallel substitution", category: "algebra" });
        functions.insert("ratsubst", DocEntry { signature: "ratsubst(a, b, expr)", doc: "Rational substitution", category: "algebra" });
        functions.insert("coeff", DocEntry { signature: "coeff(expr, var, n)", doc: "Coefficient of var^n in expr", category: "algebra" });
        functions.insert("bothcoef", DocEntry { signature: "bothcoef(expr, var)", doc: "Coefficient and remainder", category: "algebra" });
        functions.insert("solve", DocEntry { signature: "solve(expr, var)  |  solve([eqns], [vars])", doc: "Solve equation(s) for variable(s)", category: "algebra" });
        functions.insert("linsolve", DocEntry { signature: "linsolve([eqns], [vars])", doc: "Solve linear equations", category: "algebra" });
        functions.insert("allroots", DocEntry { signature: "allroots(expr)", doc: "Find all roots of a polynomial", category: "algebra" });
        functions.insert("realroots", DocEntry { signature: "realroots(expr)", doc: "Find real roots", category: "algebra" });

        // ── Matrix ──
        functions.insert("matrix", DocEntry { signature: "matrix(row1, row2, ...)", doc: "Create a matrix from rows", category: "matrix" });
        functions.insert("determinant", DocEntry { signature: "determinant(mat)", doc: "Determinant of a matrix", category: "matrix" });
        functions.insert("invert", DocEntry { signature: "invert(mat)", doc: "Inverse of a matrix", category: "matrix" });
        functions.insert("transpose", DocEntry { signature: "transpose(mat)", doc: "Transpose of a matrix", category: "matrix" });
        functions.insert("eigenvalues", DocEntry { signature: "eigenvalues(mat)", doc: "Eigenvalues of a matrix", category: "matrix" });
        functions.insert("eigenvectors", DocEntry { signature: "eigenvectors(mat)", doc: "Eigenvectors of a matrix", category: "matrix" });
        functions.insert("diagmatrix", DocEntry { signature: "diagmatrix(n, x)", doc: "Diagonal matrix of size n with x on diagonal", category: "matrix" });
        functions.insert("ident", DocEntry { signature: "ident(n)", doc: "Identity matrix of size n", category: "matrix" });
        functions.insert("zeromatrix", DocEntry { signature: "zeromatrix(m, n)", doc: "Zero matrix of size m x n", category: "matrix" });

        // ── Set operations ──
        functions.insert("union", DocEntry { signature: "union(set1, set2, ...)", doc: "Union of sets", category: "sets" });
        functions.insert("intersection", DocEntry { signature: "intersection(set1, set2, ...)", doc: "Intersection of sets", category: "sets" });
        functions.insert("setdifference", DocEntry { signature: "setdifference(set1, set2)", doc: "Set difference", category: "sets" });
        functions.insert("setp", DocEntry { signature: "setp(expr)", doc: "Test if expression is a set", category: "sets" });
        functions.insert("cardinality", DocEntry { signature: "cardinality(set)", doc: "Number of elements in a set", category: "sets" });
        functions.insert("subset", DocEntry { signature: "subset(set, pred)", doc: "Subset of set satisfying pred", category: "sets" });
        functions.insert("adjoin", DocEntry { signature: "adjoin(x, set)", doc: "Add element x to set", category: "sets" });
        functions.insert("disjoin", DocEntry { signature: "disjoin(x, set)", doc: "Remove element x from set", category: "sets" });

        // ── Number theory ──
        functions.insert("primep", DocEntry { signature: "primep(n)", doc: "Test if n is prime", category: "number theory" });
        functions.insert("next_prime", DocEntry { signature: "next_prime(n)", doc: "Next prime greater than n", category: "number theory" });
        functions.insert("prev_prime", DocEntry { signature: "prev_prime(n)", doc: "Previous prime less than n", category: "number theory" });
        functions.insert("gcd", DocEntry { signature: "gcd(a, b)", doc: "Greatest common divisor", category: "number theory" });
        functions.insert("lcm", DocEntry { signature: "lcm(a, b)", doc: "Least common multiple", category: "number theory" });
        functions.insert("mod", DocEntry { signature: "mod(a, b)", doc: "Modulus (remainder)", category: "number theory" });
        functions.insert("factor", DocEntry { signature: "factor(n)", doc: "Prime factorization of integer n", category: "number theory" });
        functions.insert("divisors", DocEntry { signature: "divisors(n)", doc: "All divisors of n", category: "number theory" });

        // ── Special functions ──
        functions.insert("sqrt", DocEntry { signature: "sqrt(x)", doc: "Square root", category: "special" });
        functions.insert("abs", DocEntry { signature: "abs(x)", doc: "Absolute value", category: "special" });
        functions.insert("log", DocEntry { signature: "log(x)", doc: "Natural logarithm", category: "special" });
        functions.insert("exp", DocEntry { signature: "exp(x)", doc: "Exponential e^x", category: "special" });
        functions.insert("gamma", DocEntry { signature: "gamma(z)", doc: "Gamma function", category: "special" });
        functions.insert("beta", DocEntry { signature: "beta(a, b)", doc: "Beta function", category: "special" });
        functions.insert("zeta", DocEntry { signature: "zeta(z)", doc: "Riemann zeta function", category: "special" });
        functions.insert("erf", DocEntry { signature: "erf(x)", doc: "Error function", category: "special" });
        functions.insert("bessel_j", DocEntry { signature: "bessel_j(n, x)", doc: "Bessel function of first kind", category: "special" });
        functions.insert("bessel_y", DocEntry { signature: "bessel_y(n, x)", doc: "Bessel function of second kind", category: "special" });

        // ── Control + programming ──
        functions.insert("block", DocEntry { signature: "block([vars], expr1, expr2, ...)", doc: "Compound expression with local variables", category: "control" });
        functions.insert("lambda", DocEntry { signature: "lambda([params], body)", doc: "Anonymous function", category: "control" });
        functions.insert("define", DocEntry { signature: "define(f(x), expr)", doc: "Define a function (evaluates body)", category: "control" });
        functions.insert("buildq", DocEntry { signature: "buildq([vars], template)", doc: "Build expression with substitution", category: "control" });
        functions.insert("splice", DocEntry { signature: "splice(list)", doc: "Splice list into expression", category: "control" });
        functions.insert("apply", DocEntry { signature: "apply(f, [args])", doc: "Apply function f to argument list", category: "control" });
        functions.insert("map", DocEntry { signature: "map(f, expr1, ...)", doc: "Apply f to each argument", category: "control" });
        functions.insert("makelist", DocEntry { signature: "makelist(expr, var, lo, hi)", doc: "Create a list", category: "control" });

        // ── Display ──
        functions.insert("print", DocEntry { signature: "print(expr1, expr2, ...)", doc: "Print expressions", category: "display" });
        functions.insert("display", DocEntry { signature: "display(expr1, expr2, ...)", doc: "Display expressions with labels", category: "display" });
        functions.insert("tex", DocEntry { signature: "tex(expr)", doc: "Output expression in TeX format", category: "display" });
        functions.insert("fortran", DocEntry { signature: "fortran(expr)", doc: "Output expression in Fortran format", category: "display" });
        functions.insert("c", DocEntry { signature: "c(expr)", doc: "Output expression in C format", category: "display" });

        // ── System ──
        functions.insert("kill", DocEntry { signature: "kill(a, b, ...)  |  kill(all)", doc: "Remove variables, functions, or labels", category: "system" });
        functions.insert("remvalue", DocEntry { signature: "remvalue(a, b, ...)", doc: "Remove values of variables", category: "system" });
        functions.insert("remfunction", DocEntry { signature: "remfunction(f1, f2, ...)", doc: "Remove function definitions", category: "system" });
        functions.insert("load", DocEntry { signature: "load(filename)", doc: "Load a Maxima package", category: "system" });
        functions.insert("batch", DocEntry { signature: "batch(filename)", doc: "Load and execute file", category: "system" });
        functions.insert("batchload", DocEntry { signature: "batchload(filename)", doc: "Load file without display", category: "system" });
        functions.insert("string", DocEntry { signature: "string(expr)", doc: "Convert expression to string", category: "system" });
        functions.insert("eval_string", DocEntry { signature: "eval_string(str)", doc: "Evaluate string as Maxima expression", category: "system" });
        functions.insert("parse_string", DocEntry { signature: "parse_string(str)", doc: "Parse string to Maxima expression", category: "system" });

        // ── Constants ──
        let mut constants = HashMap::new();
        constants.insert("%", DocEntry { signature: "%", doc: "Last result", category: "constant" });
        constants.insert("%e", DocEntry { signature: "%e", doc: "Euler's number (base of natural log)", category: "constant" });
        constants.insert("%i", DocEntry { signature: "%i", doc: "Imaginary unit sqrt(-1)", category: "constant" });
        constants.insert("%pi", DocEntry { signature: "%pi", doc: "Pi (3.14159...)", category: "constant" });
        constants.insert("%phi", DocEntry { signature: "%phi", doc: "Golden ratio (1.61803...)", category: "constant" });
        constants.insert("%gamma", DocEntry { signature: "%gamma", doc: "Euler-Mascheroni constant", category: "constant" });
        constants.insert("inf", DocEntry { signature: "inf", doc: "Positive infinity", category: "constant" });
        constants.insert("minf", DocEntry { signature: "minf", doc: "Negative infinity", category: "constant" });
        constants.insert("true", DocEntry { signature: "true", doc: "Boolean true", category: "constant" });
        constants.insert("false", DocEntry { signature: "false", doc: "Boolean false", category: "constant" });

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

        Self { functions, constants, keywords, operators }
    }
}
