int main() {
    int a;
    int *p;
    int **pp;
    a = 1;
    p = &a;
    pp = &p;
    **pp = 8;
    *p = *p + 4;
    return a;
}
