// we will add an operator for simplification similar to how we did for evaluation
// we will name it `simplify`
// we will use it on the AST
// TODO : create a DAG.rs later to get the simplest form with the least number of of AST steps
// we will go from down to up; from leaves to the root node like how we did for evaluation
// rules: [x=variable, n=number]
// x+0=x
// n+0=n
// x-0=x
// n-0=n
// x*0=0
// n*0=0
// x/0=inf [unsure whether to add]
// n/0=inf [unsure whether to add]
// x^0=1
// n^0=1
// x(0)=0
// n(0)=0
// we should also add a feature later to optimize what rules to check first, like maybe if the expression deals with brackets,
// we should check rules related to brackets first
