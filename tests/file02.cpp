#include <iostream>  
#include <vector>  
#include <string>  
  
// Global constants (changed value)  
const int kMaxItems = 200;  
  
// Simple function (unchanged)  
void greet() {  
    std::cout << "Hello, world!\n";  
}  
  
// Function with parameters and a modified comment  
int add(int a, int b) {  
    // Updated comment here  
    return a + b;  
}  
  
// Struct definition (unchanged)  
struct Point {  
    double x, y;  
};  
  
// Class with methods (added a new method)  
class Calculator {  
public:  
    Calculator(int value) : value_(value) {}  
  
    int getValue() const { return value_; }  
  
    void setValue(int v) { value_ = v; }  
  
    // New method  
    void reset() { value_ = 0; }  
  
private:  
    int value_;  
};  
  
// Main function (changed logic and added whitespace)  
int main() {  
    greet();  
    int sum = add(2, 3);  
    std::cout << "Sum: " << sum << "\n";  // Changed newline style  
    Point p{1.0, 2.0};  
    Calculator calc(42);  
    calc.reset(); // New call  
    std::cout << "Calc value: " << calc.getValue() << "\n";  
    return 0;  
}
