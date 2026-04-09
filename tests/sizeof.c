int main() {
    int a;
    int *p;
    p = &a;
    return sizeof a + sizeof p;
}
