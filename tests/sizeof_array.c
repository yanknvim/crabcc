int main() {
    int a[3];
    int *p;
    p = a;
    return sizeof a + sizeof p;
}
