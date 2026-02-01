import React, { useState, useRef, useEffect } from 'react';
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  FlatList,
  StyleSheet,
  StatusBar,
  KeyboardAvoidingView,
  Platform,
  ActivityIndicator,
} from 'react-native';
import * as Speech from 'expo-speech';

// Config - change to your Mac's IP
const API_URL = 'http://192.168.1.100:8080';

export default function App() {
  const [messages, setMessages] = useState([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isListening, setIsListening] = useState(false);
  const [voiceEnabled, setVoiceEnabled] = useState(true);
  const [serverIP, setServerIP] = useState('192.168.1.100');
  const [showSettings, setShowSettings] = useState(false);
  const flatListRef = useRef(null);

  const sendMessage = async (text) => {
    if (!text.trim()) return;

    const userMessage = { role: 'user', content: text, id: Date.now() };
    setMessages(prev => [...prev, userMessage]);
    setInput('');
    setIsLoading(true);

    try {
      const response = await fetch(`http://${serverIP}:8080/chat`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message: text }),
      });

      const data = await response.json();
      const assistantMessage = {
        role: 'assistant',
        content: data.response,
        id: Date.now() + 1,
      };

      setMessages(prev => [...prev, assistantMessage]);

      // Speak response if voice enabled
      if (voiceEnabled && data.response) {
        Speech.speak(data.response, {
          language: 'ru-RU',
          rate: 1.0,
        });
      }
    } catch (error) {
      setMessages(prev => [
        ...prev,
        {
          role: 'assistant',
          content: 'Connection error. Check server IP.',
          id: Date.now() + 1,
          error: true,
        },
      ]);
    } finally {
      setIsLoading(false);
    }
  };

  const toggleVoice = () => {
    if (voiceEnabled) {
      Speech.stop();
    }
    setVoiceEnabled(!voiceEnabled);
  };

  const renderMessage = ({ item }) => (
    <View
      style={[
        styles.messageBubble,
        item.role === 'user' ? styles.userBubble : styles.assistantBubble,
        item.error && styles.errorBubble,
      ]}
    >
      <Text
        style={[
          styles.messageText,
          item.role === 'user' ? styles.userText : styles.assistantText,
        ]}
      >
        {item.content}
      </Text>
    </View>
  );

  if (showSettings) {
    return (
      <View style={styles.container}>
        <StatusBar barStyle="light-content" backgroundColor="#000" />
        <View style={styles.header}>
          <TouchableOpacity onPress={() => setShowSettings(false)}>
            <Text style={styles.backButton}>Back</Text>
          </TouchableOpacity>
          <Text style={styles.headerTitle}>Settings</Text>
          <View style={{ width: 50 }} />
        </View>

        <View style={styles.settingsContent}>
          <Text style={styles.settingsLabel}>Server IP</Text>
          <TextInput
            style={styles.settingsInput}
            value={serverIP}
            onChangeText={setServerIP}
            placeholder="192.168.1.100"
            placeholderTextColor="#666"
            keyboardType="numeric"
          />
          <Text style={styles.settingsHint}>
            Find your Mac's IP: System Settings → Network
          </Text>
        </View>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      <StatusBar barStyle="light-content" backgroundColor="#000" />

      {/* Header */}
      <View style={styles.header}>
        <TouchableOpacity onPress={() => setShowSettings(true)}>
          <Text style={styles.settingsButton}>IP</Text>
        </TouchableOpacity>

        <Text style={styles.headerTitle}>Hanni</Text>

        <TouchableOpacity onPress={toggleVoice} style={styles.voiceToggle}>
          <Text style={styles.voiceIcon}>{voiceEnabled ? '🔊' : '🔇'}</Text>
        </TouchableOpacity>
      </View>

      {/* Messages */}
      <FlatList
        ref={flatListRef}
        data={messages}
        renderItem={renderMessage}
        keyExtractor={item => item.id.toString()}
        style={styles.messageList}
        contentContainerStyle={styles.messageListContent}
        onContentSizeChange={() => flatListRef.current?.scrollToEnd()}
      />

      {/* Loading indicator */}
      {isLoading && (
        <View style={styles.loadingContainer}>
          <ActivityIndicator color="#fff" size="small" />
          <Text style={styles.loadingText}>Thinking...</Text>
        </View>
      )}

      {/* Input */}
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      >
        <View style={styles.inputContainer}>
          <TextInput
            style={styles.input}
            value={input}
            onChangeText={setInput}
            placeholder="Message"
            placeholderTextColor="#666"
            multiline
            maxLength={1000}
          />
          <TouchableOpacity
            style={[styles.sendButton, !input.trim() && styles.sendButtonDisabled]}
            onPress={() => sendMessage(input)}
            disabled={!input.trim() || isLoading}
          >
            <Text style={styles.sendButtonText}>→</Text>
          </TouchableOpacity>
        </View>
      </KeyboardAvoidingView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingTop: 48,
    paddingBottom: 12,
    borderBottomWidth: 1,
    borderBottomColor: '#222',
  },
  headerTitle: {
    color: '#fff',
    fontSize: 17,
    fontWeight: '600',
    letterSpacing: -0.5,
  },
  settingsButton: {
    color: '#666',
    fontSize: 14,
    fontWeight: '500',
  },
  voiceToggle: {
    padding: 4,
  },
  voiceIcon: {
    fontSize: 20,
  },
  messageList: {
    flex: 1,
  },
  messageListContent: {
    padding: 16,
    gap: 8,
  },
  messageBubble: {
    maxWidth: '85%',
    padding: 12,
    marginBottom: 4,
  },
  userBubble: {
    alignSelf: 'flex-end',
    backgroundColor: '#fff',
  },
  assistantBubble: {
    alignSelf: 'flex-start',
    backgroundColor: '#1a1a1a',
    borderWidth: 1,
    borderColor: '#333',
  },
  errorBubble: {
    borderColor: '#ff4444',
  },
  messageText: {
    fontSize: 15,
    lineHeight: 22,
    letterSpacing: -0.3,
  },
  userText: {
    color: '#000',
  },
  assistantText: {
    color: '#fff',
  },
  loadingContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 8,
    gap: 8,
  },
  loadingText: {
    color: '#666',
    fontSize: 13,
  },
  inputContainer: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    padding: 12,
    gap: 8,
    borderTopWidth: 1,
    borderTopColor: '#222',
  },
  input: {
    flex: 1,
    backgroundColor: '#1a1a1a',
    color: '#fff',
    fontSize: 15,
    padding: 12,
    maxHeight: 100,
    borderWidth: 1,
    borderColor: '#333',
  },
  sendButton: {
    backgroundColor: '#fff',
    width: 44,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  sendButtonDisabled: {
    backgroundColor: '#333',
  },
  sendButtonText: {
    color: '#000',
    fontSize: 20,
    fontWeight: '600',
  },
  backButton: {
    color: '#fff',
    fontSize: 15,
  },
  settingsContent: {
    padding: 20,
  },
  settingsLabel: {
    color: '#fff',
    fontSize: 14,
    fontWeight: '600',
    marginBottom: 8,
    letterSpacing: -0.3,
  },
  settingsInput: {
    backgroundColor: '#1a1a1a',
    color: '#fff',
    fontSize: 15,
    padding: 12,
    borderWidth: 1,
    borderColor: '#333',
  },
  settingsHint: {
    color: '#666',
    fontSize: 12,
    marginTop: 8,
  },
});
