### Why PDEs Differ from ODEs

For an ODE, specifying enough initial data often determines a trajectory. A PDE also needs information about the spatial domain and its boundary.

For a domain $\Omega$, typical boundary conditions include:

**Dirichlet**

$$
u=g
\qquad
\text{on } \partial\Omega.
$$

**Neumann**

$$
\frac{\partial u}{\partial n}=g
\qquad
\text{on } \partial\Omega.
$$

**Robin**

$$
\alpha u
+
\beta\frac{\partial u}{\partial n} =
g
\qquad
\text{on } \partial\Omega.
$$
