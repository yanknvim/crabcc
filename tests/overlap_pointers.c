int main() {
    int a;
    int *p;
    int *q;
    a = 0;
    p = &a;
    q = &a;
    *p = 7;
    *q = *q + 3;
    return a;
}
