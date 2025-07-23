# \AuthenticationApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**authenticate_user**](AuthenticationApi.md#authenticate_user) | **POST** /api/v1/authenticate | 



## authenticate_user

> models::AuthResponse authenticate_user(auth_request)


Authenticates a user using configured authentication method. The payload must be a base64-encoded, RSA-encrypted JSON object with username and password fields as follows:  ```json { \"username\": \"darth_vader\", \"password\": \"I am your father\", \"pubkey\": \"...\" } ```  If the API is in development mode, it will return a static token without actual authentication.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**auth_request** | [**AuthRequest**](AuthRequest.md) | Base64-encoded, RSA-encrypted JSON containing username and password. See description for details. | [required] |

### Return type

[**models::AuthResponse**](AuthResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

