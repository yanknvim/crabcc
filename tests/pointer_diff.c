int main() {
    int a;
    int *p;
    int *q;
    p = &a;
    q = &a;
    return p - q;
}
