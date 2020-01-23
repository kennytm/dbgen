create table animal(
    {{ rownum }}
    {{ subrownum }}
    {{ @a := rand.range(0, 100) }}
);

{{ for each row of animal generate 4 rows of limb }}
create table limb(
    {{ rownum }}
    {{ @l := subrownum }}
    {{ @a }}
    {{ @toes := least(rand.range_inclusive(0, 5), rownum-1) }}
);

{{ for each row of limb generate @toes rows of toe }}
create table toe(
    {{ rownum }}
    {{ @l }}
    {{ subrownum }}
    {{ @a }}
);

{{ for each row of ANIMAL generate 1 row of HEAD }}
create table head(
    {{ rownum }}
    {{ subrownum }}
    {{ @a * 100 }}
);
