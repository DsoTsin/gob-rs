package main

import (
	"encoding/gob"
	"fmt"
	"log"
	"os"
)

func main() {
	// 1. 创建一个复杂的 map[interface{}]interface{}
	data := createSampleData()

	// 2. 将数据编码并写入文件
	filename := "data.gob"
	err := encodeAndWriteToFile(data, filename)
	if err != nil {
		log.Fatalf("编码写入文件失败: %v", err)
	}
	fmt.Printf("数据已成功写入文件: %s\n", filename)

	// 3. 从文件读取并解码数据
	decodedData, err := decodeFromFile(filename)
	if err != nil {
		log.Fatalf("从文件解码失败: %v", err)
	}

	// 4. 打印解码后的数据
	fmt.Println("\n解码后的数据:")
	printDecodedData(decodedData)
}

// createSampleData 创建示例数据
func createSampleData() map[interface{}]interface{} {
	data := make(map[interface{}]interface{})

	// 添加各种类型的键值对
	data["name"] = "张三"
	data[42] = "数字作为键"
	data[3.14] = "浮点数作为键"
	data[true] = "布尔值作为键"

	// 嵌套结构
	nestedMap := map[string]interface{}{
		"age":    25,
		"city":   "北京",
		"active": true,
	}
	data["user_info"] = nestedMap

	// 切片作为值
	data["scores"] = []int{95, 87, 92}

	// 结构体作为值
	data["point"] = struct {
		X int
		Y int
	}{X: 10, Y: 20}

	return data
}

// encodeAndWriteToFile 编码数据并写入文件
func encodeAndWriteToFile(data map[interface{}]interface{}, filename string) error {
	// 创建文件
	file, err := os.Create(filename)
	if err != nil {
		return fmt.Errorf("创建文件失败: %v", err)
	}
	defer file.Close()

	// 创建 gob 编码器
	encoder := gob.NewEncoder(file)

	// 注册可能用到的接口类型（对于基本类型通常不需要，但自定义类型需要）
	// 对于 interface{} 包含的具体类型，gob 需要知道如何编码
	// 这里我们注册一些可能用到的具体类型
	gob.Register(map[string]interface{}{})
	gob.Register([]int{})
	gob.Register(struct {
		X int
		Y int
	}{})

	// 编码并写入文件
	err = encoder.Encode(data)
	if err != nil {
		return fmt.Errorf("编码失败: %v", err)
	}

	return nil
}

// decodeFromFile 从文件读取并解码数据
func decodeFromFile(filename string) (map[interface{}]interface{}, error) {
	// 打开文件
	file, err := os.Open(filename)
	if err != nil {
		return nil, fmt.Errorf("打开文件失败: %v", err)
	}
	defer file.Close()

	// 创建 gob 解码器
	decoder := gob.NewDecoder(file)

	// 同样需要注册用到的类型
	gob.Register(map[string]interface{}{})
	gob.Register([]int{})
	gob.Register(struct {
		X int
		Y int
	}{})

	// 解码数据
	var decodedData map[interface{}]interface{}
	err = decoder.Decode(&decodedData)
	if err != nil {
		return nil, fmt.Errorf("解码失败: %v", err)
	}

	return decodedData, nil
}

// printDecodedData 打印解码后的数据
func printDecodedData(data map[interface{}]interface{}) {
	for key, value := range data {
		fmt.Printf("键: %v (%T)\n", key, key)
		fmt.Printf("值: %v (%T)\n", value, value)
		fmt.Println("---")
	}
}
