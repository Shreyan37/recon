#include <iostream>  
#include <vector>  
  
// This function calculates the sum of numbers  
int calculateSum(const std::vector<int>& numbers) {  
    int total = 0;  
    for (int num : numbers) {  
        total += num;  
    }  
    return total;  
}  
  
int main() {  
    std::vector<int> nums = {1, 2, 3};  
    int result = calculateSum(nums);  
    std::cout << "Sum: " << result << std::endl;  
    return 0;  
}
