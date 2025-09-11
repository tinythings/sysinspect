# \ModelsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_model_details**](ModelsApi.md#get_model_details) | **GET** /api/v1/model/descr | 
[**list_models**](ModelsApi.md#list_models) | **GET** /api/v1/model/names | 



## get_model_details

> models::ModelResponse get_model_details(name)


Retrieves detailed information about a specific model in the SysInspect system. The model includes its name, description, version, maintainer, and statistics about its entities, actions, constraints, and events.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**name** | **String** | Name of the model to retrieve details for | [required] |

### Return type

[**models::ModelResponse**](ModelResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_models

> models::ModelNameResponse list_models()


Lists all available models in the SysInspect system. Each model includes details such as its name, description, version, maintainer, and statistics about its entities, actions, constraints, and events.

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ModelNameResponse**](ModelNameResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

