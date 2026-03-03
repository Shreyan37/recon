#include <iostream>  
#include <vector>  
#include <string>  
  
// Global constants  
const int kMaxItems = 100;  
  
// Simple function  
void greet() {  
    std::cout << "Hello, world!\n";  
}  
  
// Function with parameters and a comment  
int add(int a, int b) {  
    // This is a comment  
    return a + b;  
}  
  
// Struct definition  
struct Point {  
    double x, y;  
};  
  
// Class with methods  
class Calculator {  
public:  
    Calculator(int value) : value_(value) {}  
  
    int getValue() const { return value_; }  
  
    void setValue(int v) { value_ = v; }  
  
private:  
    int value_;  
};  
  
// Main function  
int main() {  
    greet();  
    int sum = add(2, 3);  
    std::cout << "Sum: " << sum << '\n';  
    Point p{1.0, 2.0};  
    Calculator calc(42);  
    std::cout << "Calc value: " << calc.getValue() << '\n';  
    return 0;  
}
