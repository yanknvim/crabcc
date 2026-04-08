int factorial(int n) {
    int next;
    if (n <= 1)
        return 1;
    next = n - 1;
    return n * factorial(next);
}

int main() {
    return factorial(5);
}
