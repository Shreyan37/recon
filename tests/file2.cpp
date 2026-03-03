#include <iostream>  
#include <vector>  
  
// This function calculates the sum of numbers in a vector  
// Updated with better documentation  
int calculateSum(const std::vector<int>& numbers) {  
    int total = 0;  
    for (int num : numbers) {  
        total += num;  // Add each number to total  
    }  
    return total;  
}  
  
int main() {  
    std::vector<int> nums = {1, 2, 3, 4, 5};  
    int result = calculateSum(nums);  
    std::cout << "Result: " << result << std::endl;  
    return 0;  
}
