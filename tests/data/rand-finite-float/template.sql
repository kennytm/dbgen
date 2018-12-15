CREATE TABLE result (
    {{ rownum }}
    {{ rand.finite_f32() }}
    {{ rand.finite_f64() }}
);
